use std::{fs, io};

use rusqlite::{OpenFlags, Row, ToSql, NO_PARAMS};
use stacks::vm::types::QualifiedContractIdentifier;

use super::Error;
use crate::config::BurnchainConfig;
use crate::stacks::util_lib::db::FromColumn;
use stacks::burnchains::{
    BurnBlockIPC, BurnBlockInputChannel, BurnHeaderIPC, BurnchainBlockDownloader, BurnchainIndexer,
    Error as BurnchainError,
};
use stacks::chainstate::burn::db::DBConn;
use stacks::core::StacksEpoch;
use stacks::types::chainstate::BurnchainHeaderHash;
use stacks::util_lib::db::Error as DBError;
use stacks::util_lib::db::{query_row, u64_to_sql, FromRow};
use stacks::util_lib::db::{sqlite_open, Error as db_error};
use stacks_common::deps_common::bitcoin::util::hash::Sha256dHash;

struct DBBurnBlockInputChannel {
    output_db_path: String,
}

/// Returns true iff the header with index `header_hash` is marked as `is_canonical` in the db.
fn is_canonical(
    connection: &DBConn,
    header_hash: BurnchainHeaderHash,
) -> Result<bool, BurnchainError> {
    let row = query_row::<u64, _>(
        connection,
        "SELECT is_canonical FROM headers WHERE header_hash = ?1",
        &[&header_hash],
    )
    .expect(&format!(
        "DBBurnchainIndexer: No header found for hash: {:?}",
        &header_hash
    ))
    .expect(&format!(
        "DBBurnchainIndexer: No header found for hash: {:?}",
        &header_hash
    ));

    Ok(row != 0)
}

/// Returns a comparison between `a` and `b` in `-1, 0, 1` format.
fn compare_headers(a: &dyn BurnHeaderIPC, b: &dyn BurnHeaderIPC) -> i64 {
    if a.height() > b.height() {
        -1
    } else if a.height() < b.height() {
        1
    } else {
        // Heights are the same, compare the hashes.
        if a.header_hash() > b.header_hash() {
            -1
        } else if a.header_hash() < b.header_hash() {
            1
        } else {
            0
        }
    }
}

/// Returns the "canonical" chain tip from the rows in the db. This is the block
/// with the highest height, breaking ties by lexicographic ordering.
fn get_canonical_chain_tip(connection: &DBConn) -> Option<Box<dyn BurnHeaderIPC>> {
    match query_row::<BurnHeaderDBRow, _>(
        connection,
        "SELECT * FROM headers ORDER BY height DESC, header_hash DESC",
        NO_PARAMS,
    )
    .expect("")
    {
        Some(row) => Some(Box::new(row)),
        None => None,
    }
}

// 1) Mark all ancestors of `new_tip` as `is_canonical`.
// 2) Stop at the first node that already is marked `is_canonical`. This the `greatest common ancestor`.
// 3) Mark each node from `node_tip` (inclusive) to the `greatest common ancestor` as not `is_canonical`.
//
// Returns the height of the `greatest common ancesor`.
fn process_reorg(
    connection: &DBConn,
    new_tip: &dyn BurnHeaderIPC,
    old_tip: &dyn BurnHeaderIPC,
) -> Result<u64, BurnchainError> {
    // Step 1: Set `is_canonical` to true for ancestors of the new tip.
    let mut up_cursor = new_tip.parent_header_hash();
    let greatest_common_ancestor = loop {
        let cursor_header = match query_row::<BurnHeaderDBRow, _>(
            connection,
            "SELECT * FROM headers WHERE header_hash = ?1",
            &[&up_cursor],
        )? {
            Some(header) => header,
            None => {
                // TODO: Make this an error.
                panic!("Couldn't find `is_canonical`.")
            }
        };
        if cursor_header.is_canonical != 0 {
            // First canonical ancestor is the greatest common ancestor.
            break cursor_header;
        }

        match connection.execute(
            "UPDATE headers SET is_canonical = 1 WHERE header_hash = ?1",
            &[&up_cursor],
        ) {
            Ok(_) => {}
            Err(e) => {
                return Err(BurnchainError::DBError(db_error::SqliteError(e)));
            }
        };

        up_cursor = cursor_header.parent_header_hash;
    };

    // Step 2: Set `is_canonical` to false from the old tip (inclusive) to the greatest
    // common ancestor (exclusive).
    let mut down_cursor = old_tip.header_hash();
    loop {
        let cursor_header = match query_row::<BurnHeaderDBRow, _>(
            connection,
            "SELECT * FROM headers WHERE header_hash = ?1",
            &[&down_cursor],
        )? {
            Some(header) => header,
            None => {
                // TODO: Should this be an error?
                panic!("Do we hit here?");
            }
        };

        if cursor_header.header_hash == greatest_common_ancestor.header_hash {
            break;
        }

        match connection.execute(
            "UPDATE headers SET is_canonical = 0 WHERE header_hash = ?1",
            &[&down_cursor],
        ) {
            Ok(_) => {}
            Err(e) => {
                return Err(BurnchainError::DBError(db_error::SqliteError(e)));
            }
        };

        down_cursor = cursor_header.parent_header_hash;
    }

    Ok(greatest_common_ancestor.height)
}

/// Returns the first ancestor of `last_canonical_tip` that is marked canonical. After a re-org, this
/// can be used to find the greatest common ancestor between the new and old chain tips.
fn find_first_canonical_ancestor(
    connection: &DBConn,
    last_canonical_tip: BurnchainHeaderHash,
) -> Result<u64, BurnchainError> {
    let mut cursor = last_canonical_tip;
    loop {
        let cursor_header = match query_row::<BurnHeaderDBRow, _>(
            connection,
            "SELECT * FROM headers WHERE header_hash = ?1",
            &[&cursor],
        )? {
            Some(header) => header,
            None => {
                // TODO: Should this be an error?
                panic!("Do we hit here?");
            }
        };

        if cursor_header.is_canonical != 0 {
            return Ok(cursor_header.height);
        }

        cursor = cursor_header.parent_header_hash;
    }
}

impl BurnBlockInputChannel for DBBurnBlockInputChannel {
    fn push_block(&self, new_block: Box<dyn BurnBlockIPC>) -> Result<(), BurnchainError> {
        // Re-open the connection.
        let open_flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let connection = sqlite_open(&self.output_db_path, open_flags, true)?;

        // Decide if this new node is part of the canonical chain.
        let current_canonical_tip_opt = get_canonical_chain_tip(&connection);

        let (is_canonical, needs_reorg) = match &current_canonical_tip_opt {
            // No canonical tip so no re-org.
            None => (true, false),

            Some(current_canonical_tip) => {
                // `new_blocks` parent is the old tip, so no reorg.
                if new_block.header().parent_header_hash() == current_canonical_tip.header_hash() {
                    (true, false)
                } else {
                    // `new_block` isn't the child of the current tip. We ASSUME we have seen all blocks before now.
                    // So, this must be a different chain. Check to see if this is a longer tip.
                    let compare_result = compare_headers(
                        current_canonical_tip.as_ref(),
                        new_block.header().as_ref(),
                    );
                    if compare_result > 0 {
                        // The new block is greater than the previous tip. It is canonical, and we need a reorg.
                        (true, true)
                    } else {
                        (false, false)
                    }
                }
            }
        };

        // Insert this header.
        let params: &[&dyn ToSql] = &[
            &(new_block.header().height() as u32),
            &new_block.header().header_hash(),
            &new_block.header().parent_header_hash(),
            &(new_block.header().time_stamp() as u32),
            &(is_canonical as u32),
        ];
        match connection.execute(
            "INSERT INTO headers (height, header_hash, parent_header_hash, time_stamp, is_canonical) VALUES (?, ?, ?, ?, ?)",
            params,
        ) {
            Ok(_) => {            }
            Err(e) => {
                return Err(BurnchainError::DBError(db_error::SqliteError(e)));
            }
        };

        // Possibly process re-org in the database representation.
        if needs_reorg {
            let push_block_process_reorg = process_reorg(
                &connection,
                new_block.header().as_ref(),
                current_canonical_tip_opt
                    .as_ref()
                    .expect("Canonical tip should exist if we are doing a reorg")
                    .as_ref(),
            )?;
        }
        Ok(())
    }
}
struct DBBlockDownloader {
    db_path: String,
}

impl BurnchainBlockDownloader for DBBlockDownloader {
    fn download(
        &self,
        _header: &dyn BurnHeaderIPC,
    ) -> Result<Box<dyn BurnBlockIPC>, BurnchainError> {
        todo!()
    }
}

#[derive(Debug)]
/// Corresponds to a row in the `headers` table.
struct BurnHeaderDBRow {
    pub height: u64,
    pub header_hash: BurnchainHeaderHash,
    pub parent_header_hash: BurnchainHeaderHash,
    pub time_stamp: u64,
    pub is_canonical: u64,
}

impl BurnHeaderIPC for BurnHeaderDBRow {
    fn height(&self) -> u64 {
        self.height
    }
    fn header_hash(&self) -> BurnchainHeaderHash {
        self.header_hash
    }
    fn parent_header_hash(&self) -> BurnchainHeaderHash {
        self.parent_header_hash
    }
    fn time_stamp(&self) -> u64 {
        self.time_stamp
    }
}
impl FromRow<BurnHeaderDBRow> for BurnHeaderDBRow {
    fn from_row<'a>(row: &'a Row) -> Result<BurnHeaderDBRow, db_error> {
        let height: u32 = row.get_unwrap("height");
        let header_hash: BurnchainHeaderHash =
            BurnchainHeaderHash::from_column(row, "header_hash")?;
        let parent_header_hash: BurnchainHeaderHash =
            BurnchainHeaderHash::from_column(row, "parent_header_hash")?;
        let time_stamp: u32 = row.get_unwrap("time_stamp");
        let is_canonical: u32 = row.get_unwrap("is_canonical");

        Ok(BurnHeaderDBRow {
            height: height.into(),
            header_hash,
            parent_header_hash,
            time_stamp: time_stamp.into(),
            is_canonical: is_canonical.into(),
        })
    }
}

const DB_BURNCHAIN_SCHEMA: &'static str = &r#"
    CREATE TABLE headers(
        height INTEGER NOT NULL,
        header_hash TEXT PRIMARY KEY NOT NULL,
        parent_header_hash TEXT NOT NULL,
        time_stamp INTEGER NOT NULL,
        is_canonical INTEGER NOT NULL  -- is this block on the canonical path?
    );
    "#;

/// Tracks burnchain forks by storing the block headers in a database.
pub struct DBBurnchainIndexer {
    config: BurnchainConfig,
    connection: Option<DBConn>,
    last_canonical_tip: Option<Box<dyn BurnHeaderIPC>>,
    first_burn_header_hash: BurnchainHeaderHash,
}

impl DBBurnchainIndexer {
    pub fn new(config: BurnchainConfig) -> Result<DBBurnchainIndexer, Error> {
        let first_burn_header_hash = BurnchainHeaderHash(
            Sha256dHash::from_hex(&config.first_burn_header_hash)
                .expect("Could not parse `first_burn_header_hash`.")
                .0,
        );

        Ok(DBBurnchainIndexer {
            config,
            connection: None,
            last_canonical_tip: None,
            first_burn_header_hash,
        })
    }
}

impl BurnchainIndexer for DBBurnchainIndexer {
    fn connect(&mut self, readwrite: bool) -> Result<(), BurnchainError> {
        let path = &self.config.indexer_base_db_path;
        let mut create_flag = false;
        let open_flags = match fs::metadata(path) {
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    // need to create
                    if readwrite {
                        create_flag = true;
                        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
                    } else {
                        return Err(BurnchainError::from(DBError::NoDBError));
                    }
                } else {
                    return Err(BurnchainError::from(DBError::IOError(e)));
                }
            }
            Ok(_md) => {
                // can just open
                if readwrite {
                    OpenFlags::SQLITE_OPEN_READ_WRITE
                } else {
                    OpenFlags::SQLITE_OPEN_READ_ONLY
                }
            }
        };

        self.connection = Some(sqlite_open(path, open_flags, true)?);

        if create_flag {
            let _ = self
                .connection
                .as_ref()
                .unwrap()
                .execute(DB_BURNCHAIN_SCHEMA, NO_PARAMS)
                .map_err(|e| BurnchainError::DBError(db_error::SqliteError(e)))?;
        }

        self.last_canonical_tip = get_canonical_chain_tip(self.connection.as_ref().unwrap());

        Ok(())
    }

    fn subnets_contract(&self) -> QualifiedContractIdentifier {
        self.config.contract_identifier.clone()
    }

    fn get_input_channel(&self) -> Box<dyn BurnBlockInputChannel> {
        Box::new(DBBurnBlockInputChannel {
            output_db_path: self.get_headers_path(),
        })
    }

    fn get_first_block_height(&self) -> u64 {
        let header = self.get_header_for_hash(&self.first_burn_header_hash);
        header.height()
    }

    fn get_first_block_header_hash(&self) -> Result<BurnchainHeaderHash, BurnchainError> {
        let header = self.get_header_for_hash(&self.first_burn_header_hash);
        Ok(header.header_hash())
    }

    fn get_first_block_header_timestamp(&self) -> Result<u64, BurnchainError> {
        let header = self.get_header_for_hash(&self.first_burn_header_hash);
        Ok(header.time_stamp())
    }

    fn get_stacks_epochs(&self) -> Vec<StacksEpoch> {
        stacks::core::STACKS_EPOCHS_REGTEST.to_vec()
    }

    fn get_canonical_chain_tip(&self) -> Option<Box<dyn BurnHeaderIPC>> {
        get_canonical_chain_tip(self.connection.as_ref().unwrap())
    }

    fn get_headers_path(&self) -> String {
        self.config.indexer_base_db_path.clone()
    }

    fn get_headers_height(&self) -> Result<u64, BurnchainError> {
        let max = self.get_highest_header_height()?;
        Ok(max + 1)
    }

    fn get_highest_header_height(&self) -> Result<u64, BurnchainError> {
        match query_row::<u64, _>(
            &self.connection.as_ref().unwrap(),
            "SELECT MAX(height) FROM headers",
            NO_PARAMS,
        )? {
            Some(max) => Ok(max),
            None => Ok(0),
        }
    }

    fn find_chain_reorg(&mut self) -> Result<u64, BurnchainError> {
        let last_canonical_tip = match self.last_canonical_tip.as_ref() {
            Some(tip) => tip,
            None => {
                let new_tip = get_canonical_chain_tip(self.connection.as_ref().unwrap());
                self.last_canonical_tip = new_tip;
                return match &self.last_canonical_tip {
                    Some(tip) => Ok(tip.height()),
                    None => {
                        // TODO: Use height of `first header hash`.
                        Ok(0)
                    }
                };
            }
        };

        let still_canonical = is_canonical(
            self.connection.as_ref().unwrap(),
            last_canonical_tip.header_hash(),
        )
        .expect("Couldn't get is_canonical.");

        let result = if still_canonical {
            // No re-org, so return highest height.
            self.get_highest_header_height()
        } else {
            find_first_canonical_ancestor(
                self.connection.as_ref().unwrap(),
                last_canonical_tip.header_hash(),
            )
        };

        let current_tip = get_canonical_chain_tip(self.connection.as_ref().unwrap());
        self.last_canonical_tip = current_tip;
        result
    }

    fn sync_headers(
        &mut self,
        _start_height: u64,
        _end_height: Option<u64>,
    ) -> Result<u64, BurnchainError> {
        // We are not going to download blocks or wait here.
        // The returned result is always just the highest block known about.
        self.get_highest_header_height()
    }

    fn drop_headers(&mut self, _new_height: u64) -> Result<(), BurnchainError> {
        // Noop. We never forget headers in this implementation.
        Ok(())
    }

    fn read_headers(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<Box<dyn BurnHeaderIPC>>, BurnchainError> {
        let sql_query = "SELECT * FROM headers WHERE height >= ?1 AND height < ?2 and is_canonical = true ORDER BY height";
        let sql_args: &[&dyn ToSql] = &[&u64_to_sql(start_block)?, &u64_to_sql(end_block)?];

        let mut stmt = self
            .connection
            .as_ref()
            .unwrap()
            .prepare(sql_query)
            .map_err(|e| BurnchainError::DBError(db_error::SqliteError(e)))?;

        let mut rows = stmt
            .query(sql_args)
            .map_err(|e| BurnchainError::DBError(db_error::SqliteError(e)))?;

        // gather, but make sure we get _all_ headers
        let mut next_height = start_block;
        let mut headers: Vec<Box<dyn BurnHeaderIPC>> = vec![];
        while let Some(row) = rows
            .next()
            .map_err(|e| BurnchainError::DBError(db_error::SqliteError(e)))?
        {
            let height: u64 = u64::from_column(&row, "height")?;
            if height != next_height {
                break;
            }
            next_height += 1;

            let next_header = BurnHeaderDBRow::from_row(&row)?;
            headers.push(Box::new(next_header));
        }

        Ok(headers)
    }

    fn downloader(&self) -> Box<dyn BurnchainBlockDownloader> {
        Box::new(DBBlockDownloader {
            db_path: self.get_headers_path(),
        })
    }
}

impl DBBurnchainIndexer {
    pub fn get_header_for_hash(&self, hash: &BurnchainHeaderHash) -> Box<dyn BurnHeaderIPC> {
        let row = query_row::<BurnHeaderDBRow, _>(
            &self.connection.as_ref().unwrap(),
            "SELECT * FROM headers WHERE burn_header_hash = ?1",
            &[&hash],
        )
        .expect(&format!(
            "DBBurnchainIndexer: No header found for hash: {:?}",
            &hash
        ))
        .expect(&format!(
            "DBBurnchainIndexer: No header found for hash: {:?}",
            &hash
        ));

        Box::new(row)
    }
}
