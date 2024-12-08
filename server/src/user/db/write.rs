use crate::{
    authentication::types::JwtRole, postgres::PostgresClient, shared::common_types::EvmAddress,
};

impl PostgresClient {
    pub async fn edit_user(
        &self,
        address: &EvmAddress,
        new_role: &JwtRole,
    ) -> Result<(), tokio_postgres::Error> {
        self.execute(
            "
                    UPDATE user
                    SET role = $2
                    WHERE address = $1;
                ",
            &[address, new_role],
        )
        .await?;

        Ok(())
    }

    pub async fn add_user(
        &self,
        address: &EvmAddress,
        role: &JwtRole,
    ) -> Result<(), tokio_postgres::Error> {
        self.execute(
            "
                   INSERT INTO \"user\" (address, role)
                   VALUES ($1, $2)
                   ON CONFLICT DO NOTHING;
                ",
            &[address, role],
        )
        .await?;

        Ok(())
    }

    pub async fn add_users(
        &self,
        users: &Vec<(&EvmAddress, JwtRole)>,
    ) -> Result<(), tokio_postgres::Error> {
        for (address, role) in users {
            self.add_user(address, role).await?;
        }

        Ok(())
    }

    pub async fn delete_user(&self, address: &EvmAddress) -> Result<(), tokio_postgres::Error> {
        self.execute(
            "
                   DELETE FROM user
                   WHERE address = $1;
                ",
            &[address],
        )
        .await?;

        Ok(())
    }
}
