use tokio_core::reactor::Core;
use hyper::Client;
use hyper::{Method, Request, StatusCode, Error as HyperError};
use hyper::header::{ContentLength, ContentType};
use futures::{future, Future};
use serde_json;

use TolstoyResult;

use uuid;

static API_VERSION: &str = "0.1";

#[derive(Serialize)]
struct SerializedHead {
    head: uuid::Uuid
}

#[derive(Serialize)]
struct SerializedTransaction {
    parent: uuid::Uuid,
    chunks: Vec<uuid::Uuid>
}

struct RemoteClient {
    base_uri: String,
    user_id: String
}

impl RemoteClient {
    fn new(base_uri: String, user_id: String) -> Self {
        RemoteClient {
            base_uri: base_uri,
            user_id: user_id
        }
    }
}

struct RemoteHeadClient<T: RemoteClientInterface> {
    client: T
}

impl RemoteHeadClient<RemoteClient> {
    fn new(client: RemoteClient) -> Self {
        RemoteHeadClient {
            client: client
        }
    }
}

struct RemoteTransactionClient<T: RemoteClientInterface> {
    client: T
}

impl RemoteTransactionClient<RemoteClient> {
    fn new(client: RemoteClient) -> Self {
        RemoteTransactionClient {
            client: client
        }
    }
}

struct RemoteChunkClient<T: RemoteClientInterface> {
    client: T
}

impl RemoteChunkClient<RemoteClient> {
    fn new(client: RemoteClient) -> Self {
        RemoteChunkClient {
            client: client
        }
    }
}

trait RemoteClientInterface {
    fn bound_base_uri(&self) -> String;
    fn put(&self, uri: String, payload: String, expected: StatusCode) -> TolstoyResult;
}

trait RemoteTransactionClientInterface {
    fn put_transaction(&self, transaction_uuid: uuid::Uuid, parent_uuid: uuid::Uuid, chunks: Vec<uuid::Uuid>) -> TolstoyResult;
    fn serialize_transaction(&self, transaction: &SerializedTransaction) -> String;
}

trait RemoteChunkClientInterface {
    fn put_chunk(&self, chunk_uuid: uuid::Uuid, payload: String) -> TolstoyResult;
}

trait RemoteHeadClientInterface {
    fn put_head(&self, uuid: uuid::Uuid) -> TolstoyResult;
    fn serialize_head(&self, head: &SerializedHead) -> String;
}

impl RemoteClientInterface for RemoteClient {
    fn bound_base_uri(&self) -> String {
        format!("{}/{}/{}", self.base_uri, API_VERSION, self.user_id)
    }

    fn put(&self, uri: String, payload: String, expected: StatusCode) -> TolstoyResult {
        let mut core = Core::new()?;
        let client = Client::new(&core.handle());

        let uri = uri.parse()?;

        let mut req = Request::new(Method::Put, uri);
        req.headers_mut().set(ContentType::json());
        req.headers_mut().set(ContentLength(payload.len() as u64));
        req.set_body(payload);

        let put = client.request(req).and_then(|res| {
            let status_code = res.status();

            if status_code != expected {
                future::err(HyperError::Status)
            } else {
                // body will be empty...
                future::ok(())
            }
        });

        core.run(put)?;
        Ok(())
    }
}

impl RemoteHeadClientInterface for RemoteHeadClient<RemoteClient> {
    fn put_head(&self, uuid: uuid::Uuid) -> TolstoyResult {
        // {"head": uuid}
        let head = SerializedHead {
            head: uuid
        };

        let uri = format!("{}/head", self.client.bound_base_uri());
        
        let json = match serde_json::to_string(&head) {
            Ok(json) => json,
            Err(e) => panic!("Could not serialize HEAD: {}", e)
        };

       self.client.put(uri, json, StatusCode::NoContent)
    }

    fn serialize_head(&self, head: &SerializedHead) -> String {
        match serde_json::to_string(head) {
            Ok(json) => json,
            Err(e) => panic!("Could not serialize head: {}", e)
        }
    }
}

impl RemoteTransactionClientInterface for RemoteTransactionClient<RemoteClient> {
    fn put_transaction(&self, transaction_uuid: uuid::Uuid, parent_uuid: uuid::Uuid, chunks: Vec<uuid::Uuid>) -> TolstoyResult {
        // {"parent": uuid, "chunks": [chunk1, chunk2...]}
        let transaction = SerializedTransaction {
            parent: parent_uuid,
            chunks: chunks
        };

        let uri = format!("{}/transactions/{}", self.client.bound_base_uri(), transaction_uuid);
        let json = self.serialize_transaction(&transaction);

        self.client.put(uri, json, StatusCode::Created)
    }

    fn serialize_transaction(&self, transaction: &SerializedTransaction) -> String {
        match serde_json::to_string(transaction) {
            Ok(json) => json,
            Err(e) => panic!("Could not serialize transaction: {}", e)
        }
    }
}

impl RemoteChunkClientInterface for RemoteChunkClient<RemoteClient> {
    fn put_chunk(&self, chunk_uuid: uuid::Uuid, payload: String) -> TolstoyResult {
        let uri = format!("{}/chunks/{}", self.client.bound_base_uri(), chunk_uuid);
        self.client.put(uri, payload, StatusCode::Created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_remote_client(uri: &str, user_id: &str) -> RemoteClient {
        RemoteClient::new(uri.into(), user_id.into())
    }

    #[test]
    fn test_remote_client_bound_uri() {
        let remote_client = test_remote_client("https://example.com/api", "test-user");
        assert_eq!("https://example.com/api/0.1/test-user", remote_client.bound_base_uri());
    }

    // Serialization tests of questionable value...
    #[test]
    fn test_remote_client_serialize_head() {
        let remote_client = test_remote_client("https://example.com/api", "test-user");
        let remote_head_client = RemoteHeadClient::new(remote_client);
        let head = SerializedHead {
            head: uuid::Uuid::nil()
        };
        assert_eq!(
            r#"{"head":"00000000-0000-0000-0000-000000000000"}"#,
            remote_head_client.serialize_head(&head)
        );
    }

    #[test]
    fn test_remote_client_serialize_transaction() {
        let remote_client = test_remote_client("https://example.com/api", "test-user");
        let remote_transactions_client = RemoteTransactionClient::new(remote_client);
        let transaction = SerializedTransaction {
            parent: uuid::Uuid::nil(),
            chunks: vec![uuid::Uuid::nil(), uuid::Uuid::nil()]
        };
        assert_eq!(
            r#"{"parent":"00000000-0000-0000-0000-000000000000","chunks":["00000000-0000-0000-0000-000000000000","00000000-0000-0000-0000-000000000000"]}"#,
            remote_transactions_client.serialize_transaction(&transaction)
        );
    }
}
