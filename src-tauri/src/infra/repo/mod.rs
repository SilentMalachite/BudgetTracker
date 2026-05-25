//! SQL access split by table. Each repo takes `&rusqlite::Connection`
//! (or `&mut Connection` when starting a transaction) and returns
//! domain types defined in `crate::domain`.
//!
//! Repos do not lock the `Mutex<Connection>` themselves. The command layer
//! does that and passes a borrowed connection.
