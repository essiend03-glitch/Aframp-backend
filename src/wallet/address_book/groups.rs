use super::models::*;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

pub struct GroupRepository {
    pool: PgPool,
}

impl GroupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new address group
    pub async fn create_group(
        &self,
        owner_wallet_id: Uuid,
        group_name: String,
        group_description: Option<String>,
    ) -> Result<AddressGroup, sqlx::Error> {
        let group = sqlx::query_as::<_, AddressGroup>(
            r#"
            INSERT INTO address_groups
            (id, owner_wallet_id, group_name, group_description, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(owner_wallet_id)
        .bind(group_name)
        .bind(group_description)
        .fetch_one(&self.pool)
        .await?;

        Ok(group)
    }

    /// Get group by ID
    pub async fn get_group(
        &self,
        group_id: Uuid,
        owner_wallet_id: Uuid,
    ) -> Result<Option<AddressGroup>, sqlx::Error> {
        let group = sqlx::query_as::<_, AddressGroup>(
            "SELECT * FROM address_groups WHERE id = $1 AND owner_wallet_id = $2",
        )
        .bind(group_id)
        .bind(owner_wallet_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(group)
    }

    /// List all groups for a wallet
    pub async fn list_groups(
        &self,
        owner_wallet_id: Uuid,
    ) -> Result<Vec<AddressGroup>, sqlx::Error> {
        let groups = sqlx::query_as::<_, AddressGroup>(
            "SELECT * FROM address_groups WHERE owner_wallet_id = $1 ORDER BY created_at DESC",
        )
        .bind(owner_wallet_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    /// Update group
    pub async fn update_group(
        &self,
        group_id: Uuid,
        owner_wallet_id: Uuid,
        group_name: Option<String>,
        group_description: Option<String>,
    ) -> Result<AddressGroup, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new("UPDATE address_groups SET updated_at = NOW()");

        if let Some(name) = group_name {
            query.push(", group_name = ");
            query.push_bind(name);
        }

        if group_description.is_some() {
            query.push(", group_description = ");
            query.push_bind(group_description);
        }

        query.push(" WHERE id = ");
        query.push_bind(group_id);
        query.push(" AND owner_wallet_id = ");
        query.push_bind(owner_wallet_id);
        query.push(" RETURNING *");

        let group = query
            .build_query_as::<AddressGroup>()
            .fetch_one(&self.pool)
            .await?;

        Ok(group)
    }

    /// Delete group
    pub async fn delete_group(
        &self,
        group_id: Uuid,
        owner_wallet_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        // First remove all memberships
        sqlx::query("DELETE FROM group_memberships WHERE group_id = $1")
            .bind(group_id)
            .execute(&self.pool)
            .await?;

        // Then delete the group
        sqlx::query("DELETE FROM address_groups WHERE id = $1 AND owner_wallet_id = $2")
            .bind(group_id)
            .bind(owner_wallet_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Add entries to group
    pub async fn add_members(
        &self,
        group_id: Uuid,
        entry_ids: Vec<Uuid>,
    ) -> Result<usize, sqlx::Error> {
        let mut added = 0;

        for entry_id in entry_ids {
            let result = sqlx::query(
                r#"
                INSERT INTO group_memberships (group_id, entry_id, added_at)
                VALUES ($1, $2, NOW())
                ON CONFLICT (group_id, entry_id) DO NOTHING
                "#,
            )
            .bind(group_id)
            .bind(entry_id)
            .execute(&self.pool)
            .await?;

            added += result.rows_affected() as usize;
        }

        Ok(added)
    }

    /// Remove entry from group
    pub async fn remove_member(
        &self,
        group_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM group_memberships WHERE group_id = $1 AND entry_id = $2")
            .bind(group_id)
            .bind(entry_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get member count for a group
    pub async fn get_member_count(&self, group_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM group_memberships WHERE group_id = $1",
        )
        .bind(group_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get groups for an entry
    pub async fn get_entry_groups(&self, entry_id: Uuid) -> Result<Vec<AddressGroup>, sqlx::Error> {
        let groups = sqlx::query_as::<_, AddressGroup>(
            r#"
            SELECT g.* FROM address_groups g
            INNER JOIN group_memberships gm ON g.id = gm.group_id
            WHERE gm.entry_id = $1
            ORDER BY g.group_name
            "#,
        )
        .bind(entry_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    /// Count groups by owner
    pub async fn count_groups_by_owner(&self, owner_wallet_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM address_groups WHERE owner_wallet_id = $1",
        )
        .bind(owner_wallet_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Count members in a group
    pub async fn count_members_in_group(&self, group_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM group_memberships WHERE group_id = $1",
        )
        .bind(group_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }
}
