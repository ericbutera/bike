use sea_orm::{ActiveModelBehavior, ConnectionTrait, EntityTrait, IntoActiveModel};

/// Insert an ActiveModel fixture after applying overrides with struct update syntax.
pub async fn insert<A>(db: &impl ConnectionTrait, fixture: A) -> <A::Entity as EntityTrait>::Model
where
    A: ActiveModelBehavior + Send,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
{
    fixture.insert(db).await.expect("insert test fixture")
}

/// Persist a fixture patch without populating omitted fields from creation defaults.
pub async fn update<A>(db: &impl ConnectionTrait, patch: A) -> <A::Entity as EntityTrait>::Model
where
    A: ActiveModelBehavior + Send,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
{
    patch.update(db).await.expect("update test fixture")
}
