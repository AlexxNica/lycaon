use std;
use std::path::Path;
use std::sync::{Arc, Mutex};

use grpcio;
use grpc;

use failure::Error;
use futures::Future;
use uuid::Uuid;

use errors;

/// Struct implementing callbacks for the Frontend
///
/// _uploads_: a HashSet of all uuids that are currently being tracked
#[derive(Clone)]
pub struct BackendService {
    uploads: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl BackendService {
    pub fn new() -> Self {
        BackendService { uploads: Arc::new(Mutex::new(std::collections::HashSet::new())) }
    }
}

struct Layer<'a> {
    name: &'a str,
    repo: &'a str,
    digest: &'a str,
}

fn process(layer: Layer) -> Result<u64, Error> {
    let path = construct_absolute_path(layer)?;
    std::fs::metadata(path.as_os_str())?;
    debug!("File {:?} Exists", path.as_os_str());
    let file = std::fs::File::open(path)?;
    file_length(file)

}

/// Delete a file if we want
pub fn delete_blob_by_uuid(uuid: &str) -> bool {
    use std::fs;
    let path = format!("data/scratch/{}", uuid);

    fs::remove_file(path).map(|_| true).unwrap_or(false)
}

/// Takes the digest, and constructs an absolute pathstring to the digest.
fn construct_absolute_path(layer: Layer) -> Result<Box<Path>, Error> {
    std::env::current_dir()
        .map(|cwd| {
            let absolute_dir = cwd.join(format!("data/layers/{}", layer.digest));
            debug!("Absolute Path: {:?}", absolute_dir);
            absolute_dir.into_boxed_path()
        })
        .map_err(|e| e.into())
}

fn file_length(file: std::fs::File) -> Result<u64, Error> {
    file.metadata()
        .and_then(|metadata| Ok(metadata.len()))
        .map_err(|e| e.into())
}

impl grpc::backend_grpc::Backend for BackendService {
    fn layer_exists(
        &self,
        ctx: grpcio::RpcContext,
        req: grpc::backend::Layer,
        sink: grpcio::UnarySink<grpc::backend::LayerExistsResult>,
    ) {
        let layer = Layer {
            name: req.get_name(),
            repo: req.get_repo(),
            digest: req.get_digest(),
        };

        let mut resp = grpc::backend::LayerExistsResult::new();
        let _ = process(layer)
            .map(|length| {
                debug!("Success, building return object");
                resp.set_success(true);
                resp.set_length(length);
            })
            .map_err(|e| {
                debug!("Failure, building return object");
                resp.set_success(false);
            });

        let req = req.clone();
        let f = sink.success(resp).map_err(move |e| {
            warn!("failed to reply! {:?}, {:?}", req, e)
        });
        ctx.spawn(f);
    }

    fn gen_uuid(
        &self,
        ctx: grpcio::RpcContext,
        req: grpc::backend::Layer,
        sink: grpcio::UnarySink<grpc::backend::GenUuidResult>,
    ) {
        let mut resp = grpc::backend::GenUuidResult::new();
        let uuid = gen_uuid().to_string();
        {
            self.uploads.lock().unwrap().insert(uuid.clone());
            debug!("Hash Table: {:?}", self.uploads);
        }
        resp.set_uuid(uuid);
        let f = sink.success(resp).map_err(
            move |e| warn!("failed to reply! {:?}", e),
        );
        ctx.spawn(f);
    }

    fn uuid_exists(
        &self,
        ctx: grpcio::RpcContext,
        req: grpc::backend::GenUuidResult,
        sink: grpcio::UnarySink<grpc::backend::Result>,
    ) {
        let mut resp = grpc::backend::Result::new();
        let set = self.uploads.lock().unwrap();
        resp.set_success(set.contains(req.get_uuid()));

        let f = sink.success(resp).map_err(
            move |e| warn!("failed to reply! {:?}", e),
        );
        ctx.spawn(f);
    }

    fn cancel_upload(
        &self,
        ctx: grpcio::RpcContext,
        req: grpc::backend::Layer,
        sink: grpcio::UnarySink<grpc::backend::Result>,
    ) {
        let mut resp = grpc::backend::Result::new();
        let mut set = self.uploads.lock().unwrap();
        let uuid = req.get_digest();
        let _ = delete_blob_by_uuid(uuid);
        resp.set_success(set.remove(uuid));

        let f = sink.success(resp).map_err(
            move |e| warn!("failed to reply! {:?}", e),
        );
        ctx.spawn(f);
    }

    fn get_uuids(
        &self,
        ctx: grpcio::RpcContext,
        req: grpc::backend::Empty,
        sink: grpcio::UnarySink<grpc::backend::UuidList>,
    ) {
        let mut resp = grpc::backend::UuidList::new();
        {
            use protobuf;
            use std::iter::FromIterator;
            let set = self.uploads.lock().unwrap();
            let set = set.clone().into_iter().map(|x| {
                let mut val = grpc::backend::GenUuidResult::new();
                val.set_uuid(x);
                val
            });
            resp.set_uuids(protobuf::RepeatedField::from_iter(set));
        }
        let f = sink.success(resp).map_err(
            move |e| warn!("failed to reply! {:?}", e),
        );
        ctx.spawn(f);
    }

    fn commit_upload(
        &self,
        ctx: grpcio::RpcContext,
        req: grpc::backend::CommitLayer,
        sink: grpcio::UnarySink<grpc::backend::Result>,
    ) {
        do_thing(req);
        let mut resp = grpc::backend::Result::new();
        let f = sink.success(resp).map_err(
            move |e| warn!("failed to reply! {:?}", e),
        );
        ctx.spawn(f);
    }
}

fn do_thing(req: grpc::backend::CommitLayer) -> Result<(), Error> {
    use std::fs;
    use ring::digest;
    use std::io::Read;

    let uuid = req.get_uuid();
    let digest = req.get_digest();
    let uuid_path = format!("data/scratch/{}", uuid);
    let digest_path = format!("data/layers/{}", digest);
    let mut vec_file = &mut Vec::new();

    let mut file = fs::File::open(&uuid_path)?;
    file.read_to_end(&mut vec_file);
    let sha = digest::digest(&digest::SHA256, &vec_file);

    let sha_ref: &[u8] = sha.as_ref();
    let digest_ref: &[u8] = digest.as_ref();

    if sha_ref == digest_ref {
        warn!("Digests do not match!");
        // let x: Result<(), Error> = Err(()).map_err(|e| Error::from(errors::Server::TestError(e)));
    }

    fs::rename(uuid_path, digest_path)?;
    Ok(())
}

fn gen_uuid() -> Uuid {
    Uuid::new_v4()
}
