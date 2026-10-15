//! Fixture timestamp defaults retain the connection's SQLx binding format.
use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, Schema};
pub fn table_with_timestamp_defaults<E: EntityTrait>(
    db: &impl ConnectionTrait,
    entity: E,
    columns: &[&str],
) -> sea_orm::sea_query::TableCreateStatement {
    use sea_orm::sea_query::{Expr, Table};
    let schema = Schema::new(db.get_database_backend());
    let source = schema.create_table_from_entity(entity);
    // SQLite compares timestamp text; SQLx binds UTC values as RFC 3339.
    let clock = if db.get_database_backend() == DbBackend::Sqlite {
        Expr::cust("strftime('%Y-%m-%dT%H:%M:%S+00:00', 'now')")
    } else {
        Expr::current_timestamp()
    };
    let mut table = Table::create();
    table.table(entity);
    for column in source.get_columns() {
        let mut column = column.clone();
        if columns.contains(&column.get_column_name().as_str()) {
            column.default(clock.clone());
        }
        table.col(&mut column);
    }
    table.to_owned()
}
