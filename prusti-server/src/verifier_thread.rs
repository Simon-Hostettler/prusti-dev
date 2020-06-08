use super::VerifierRunner;
use futures::Canceled;
use futures::{sync::oneshot, Future};
use prusti_viper::encoder::vir::Program;
use prusti_viper::verifier::VerifierBuilder;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use viper::{VerificationBackend, VerificationResult};

pub type FutVerificationResult = Box<Future<Item = VerificationResult, Error = Canceled>>;

struct VerificationRequest {
    pub program: Program,
    pub sender: oneshot::Sender<VerificationResult>,
}

pub struct VerifierThread {
    request_sender: Mutex<mpsc::Sender<VerificationRequest>>,
}

impl VerifierThread {
    pub fn new(verifier_builder: Arc<VerifierBuilder>, backend: VerificationBackend) -> Self {
        let (request_sender, request_receiver) = mpsc::channel::<VerificationRequest>();

        let builder = thread::Builder::new().name(format!("Verifier thread running {}", backend));

        builder
            .spawn(move || {
                VerifierRunner::with_runner(&verifier_builder, backend, |runner| {
                    Self::listen_for_requests(runner, request_receiver)
                });
            })
            .unwrap();

        Self {
            request_sender: Mutex::new(request_sender),
        }
    }

    fn listen_for_requests(
        runner: VerifierRunner,
        request_receiver: mpsc::Receiver<VerificationRequest>,
    ) {
        while let Ok(request) = request_receiver.recv() {
            let result = runner.verify(request.program, "for now"); // FIXME: actual name
            request.sender.send(result).unwrap_or_else(|err| {
                panic!(
                    "verifier thread attempting to send result to dropped receiver: {:?}",
                    err
                );
            });
        }
    }

    pub fn verify(&self, program: Program) -> FutVerificationResult {
        let (tx, rx) = oneshot::channel();
        self.request_sender
            .lock()
            .unwrap()
            .send(VerificationRequest {
                program,
                sender: tx,
            })
            .unwrap();
        Box::new(rx)
    }
}
