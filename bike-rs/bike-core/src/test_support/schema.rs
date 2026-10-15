//! SeaORM test tables retain timestamp defaults supplied by production migrations.
use sea_orm::{EntityTrait, Schema};
pub fn table_with_timestamp_defaults<E: EntityTrait>(
    schema: &Schema,
    entity: E,
    columns: &[&str],
) -> sea_orm::sea_query::TableCreateStatement {
    use sea_orm::sea_query::{Expr, Table};
    let source = schema.create_table_from_entity(entity);
    let mut table = Table::create();
    table.table(entity);
    for column in source.get_columns() {
        let mut column = column.clone();
        if columns.contains(&column.get_column_name().as_str()) {
            column.default(Expr::current_timestamp());
        }
        table.col(&mut column);
    }
    table.to_owned()
}
