use minio::s3::client::Client;
use minio::s3::creds::StaticProvider;
use minio::s3::error::Error as MinioError;
use r2d2::ManageConnection;

pub(crate) struct MinioConnectionManager {
    endpoint: String,
    access_key: String,
    secret_key: String,
}

impl MinioConnectionManager {
    pub fn new(endpoint: String, access_key: String, secret_key: String) -> Self {
        MinioConnectionManager {
            endpoint,
            access_key,
            secret_key,
        }
    }
}

impl ManageConnection for MinioConnectionManager {
    type Connection = Client;
    type Error = MinioError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let base_url = self.endpoint.parse()
            .map_err(|e| MinioError::from(e))?;  // 修改这里
        let provider = StaticProvider::new(&self.access_key, &self.secret_key, None);
        let client = Client::new(base_url, Some(Box::new(provider)), None, None)
            .map_err(|e| MinioError::from(e))?;  // 如果需要，修改这里
        Ok(client)
    }


    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        // Implement logic to verify connection is still valid
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        // Implement logic to check if connection is broken
        false
    }
}

