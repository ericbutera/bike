use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ActivityRouteBounds {
    Table,
    ActivityId,
}

#[derive(DeriveIden)]
enum SegmentRouteBounds {
    Table,
    SegmentId,
}

#[derive(DeriveIden)]
enum Sources {
    Activities,
    Segments,
    Id,
}

#[derive(DeriveIden)]
enum Bounds {
    PointCount,
    MinLatitude,
    MaxLatitude,
    MinLongitude,
    MaxLongitude,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(route_bounds(
                ActivityRouteBounds::Table.into_iden(),
                ActivityRouteBounds::ActivityId.into_iden(),
                Sources::Activities.into_iden(),
            ))
            .await?;
        manager
            .create_table(route_bounds(
                SegmentRouteBounds::Table.into_iden(),
                SegmentRouteBounds::SegmentId.into_iden(),
                Sources::Segments.into_iden(),
            ))
            .await?;
        manager
            .create_index(spatial_index(
                ActivityRouteBounds::Table,
                "activity_route_bounds_spatial",
            ))
            .await?;
        manager
            .create_index(spatial_index(SegmentRouteBounds::Table, "segment_route_bounds_spatial"))
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SegmentRouteBounds::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ActivityRouteBounds::Table).to_owned())
            .await
    }
}

fn route_bounds(table: DynIden, owner_id: DynIden, source: DynIden) -> TableCreateStatement {
    Table::create()
        .table(table.clone())
        .col(ColumnDef::new(owner_id.clone()).integer().not_null().primary_key())
        .col(ColumnDef::new(Bounds::PointCount).big_integer().not_null())
        .col(ColumnDef::new(Bounds::MinLatitude).double())
        .col(ColumnDef::new(Bounds::MaxLatitude).double())
        .col(ColumnDef::new(Bounds::MinLongitude).double())
        .col(ColumnDef::new(Bounds::MaxLongitude).double())
        .check(Expr::col(Bounds::PointCount).gte(0))
        .check(Condition::any().add(empty_bounds()).add(populated_bounds()))
        .foreign_key(
            ForeignKey::create()
                .from(table, owner_id)
                .to(source, Sources::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn empty_bounds() -> Condition {
    Condition::all()
        .add(Expr::col(Bounds::PointCount).lt(2))
        .add(Expr::col(Bounds::MinLatitude).is_null())
        .add(Expr::col(Bounds::MaxLatitude).is_null())
        .add(Expr::col(Bounds::MinLongitude).is_null())
        .add(Expr::col(Bounds::MaxLongitude).is_null())
}

fn populated_bounds() -> Condition {
    Condition::all()
        .add(Expr::col(Bounds::PointCount).gte(2))
        .add(Expr::col(Bounds::MinLatitude).is_not_null())
        .add(Expr::col(Bounds::MaxLatitude).is_not_null())
        .add(Expr::col(Bounds::MinLongitude).is_not_null())
        .add(Expr::col(Bounds::MaxLongitude).is_not_null())
        .add(Expr::col(Bounds::MinLatitude).gte(-90))
        .add(Expr::col(Bounds::MaxLatitude).lte(90))
        .add(Expr::col(Bounds::MinLatitude).lte(Expr::col(Bounds::MaxLatitude)))
        .add(Expr::col(Bounds::MinLongitude).gte(-180))
        .add(Expr::col(Bounds::MaxLongitude).lte(180))
        .add(Expr::col(Bounds::MinLongitude).lte(Expr::col(Bounds::MaxLongitude)))
}

fn spatial_index(table: impl IntoTableRef, name: &str) -> IndexCreateStatement {
    let minimum =
        Func::cust(Alias::new("point")).args([Expr::col(Bounds::MinLongitude), Expr::col(Bounds::MinLatitude)]);
    let maximum =
        Func::cust(Alias::new("point")).args([Expr::col(Bounds::MaxLongitude), Expr::col(Bounds::MaxLatitude)]);
    Index::create()
        .name(name)
        .table(table)
        .index_type(IndexType::Custom(Alias::new("gist").into_iden()))
        .col(Func::cust(Alias::new("box")).args([minimum.into(), maximum.into()]))
        .and_where(Expr::col(Bounds::PointCount).gte(2))
        .to_owned()
}
