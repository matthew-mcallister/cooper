//! Implements a multithreaded request--response model over channels.

use crossbeam_channel as cc;
use derive_more::*;

#[derive(Debug, Display)]
pub enum RequestError<T> {
    /// The channel was full.
    Busy(T),
    /// The channel was closed before the request could be sent.
    #[display(fmt = "failed sending message: {}", "_0")]
    Unavailable(T),
    /// The channel was closed while waiting for a response.
    #[display(fmt = "response channel disconnected")]
    Interrupted,
    /// The operation timed out.
    #[display(fmt = "operation timed out")]
    Timeout,
}

impl<T, U> From<cc::SendError<Message<T, U>>> for RequestError<T> {
    fn from(err: cc::SendError<Message<T, U>>) -> Self {
        RequestError::Unavailable(err.0.payload)
    }
}

impl<T, U> From<cc::TrySendError<Message<T, U>>> for RequestError<T> {
    fn from(err: cc::TrySendError<Message<T, U>>) -> Self {
        match err {
            cc::TrySendError::Full(msg) =>
                RequestError::Busy(msg.payload),
            cc::TrySendError::Disconnected(msg) =>
                RequestError::Unavailable(msg.payload),
        }
    }
}

#[derive(Debug)]
struct Message<T, U> {
    payload: T,
    res_chan: Option<cc::Sender<U>>,
}

type Message2<H> = Message<
    <H as RequestHandler>::Request,
    <H as RequestHandler>::Response,
>;

#[derive(Clone, Debug)]
pub struct RequestSender<T, U> {
    inner: cc::Sender<Message<T, U>>,
}

pub trait RequestHandler {
    type Request;
    type Response;

    fn handle(&mut self, request: Self::Request) -> Option<Self::Response>;
}

#[derive(Debug)]
pub struct Service<H: RequestHandler> {
    receiver: cc::Receiver<Message<H::Request, H::Response>>,
    handler: H,
}

impl<T, U> RequestSender<T, U> {
    /// Sends a request and ignores the result.
    pub fn send(&self, payload: T) -> Result<(), RequestError<T>> {
        let request = Message {
            payload,
            res_chan: None,
        };
        Ok(self.inner.send(request)?)
    }

    /// Sends a request and awaits the result.
    pub fn wait_on(&self, payload: T) -> Result<U, RequestError<T>> {
        // Use zero-sized queue since we immediately wait on it.
        let (sender, receiver) = cc::bounded(0);
        let request = Message {
            payload,
            res_chan: Some(sender),
        };
        self.inner.send(request)?;
        Ok(receiver.recv().map_err(|_| RequestError::Interrupted)?)
    }
}

impl<H: RequestHandler> Service<H> {
    pub fn into_handler(self) -> H {
        self.handler
    }

    pub fn unbounded(handler: H) ->
        (Self, RequestSender<H::Request, H::Response>)
    {
        let (sender, receiver) = cc::unbounded::<Message<_, _>>();
        let sender = RequestSender { inner: sender };
        let service = Service {
            receiver,
            handler,
        };
        (service, sender)
    }

    fn handle_request(&mut self, req: Message2<H>) {
        let res = self.handler.handle(req.payload);
        let _: Option<_> = try { req.res_chan?.send(res?) };
    }

    /// Handles waiting requests, if any.
    pub fn try_pump(&mut self) -> Result<(), cc::TryRecvError>
    {
        let req = self.receiver.try_recv()?;
        self.handle_request(req);
        Ok(())
    }

    /// Pumps requests until the channel is disconnected.
    pub fn pump(&mut self) {
        while let Ok(req) = self.receiver.recv() {
            self.handle_request(req);
        }
    }

    /// Pumps messages for the specified duration. Returns `Ok` if the
    /// timeout was reached and `Err` if the channel was disconnected.
    pub fn pump_with_timeout(&mut self) -> Result<(), cc::RecvError> {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[derive(Debug)]
    enum TrivialRequest {
        Add(u32, u32),
        Record(u32),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TrivialResponse {
        Sum(u32),
    }

    #[derive(Debug, Default)]
    struct TrivialHandler {
        record: Vec<u32>,
    }

    impl RequestHandler for TrivialHandler {
        type Request = TrivialRequest;
        type Response = TrivialResponse;

        fn handle(&mut self, request: Self::Request) -> Option<Self::Response>
        {
            match request {
                TrivialRequest::Add(a, b) =>
                    return Some(TrivialResponse::Sum(a + b)),
                TrivialRequest::Record(x) => self.record.push(x),
            }
            None
        }
    }

    #[test]
    fn smoke_test() {
        let handler = TrivialHandler::default();
        let (mut service, sender) = Service::unbounded(handler);

        let thread = thread::spawn(move || {
            let res = sender.wait_on(TrivialRequest::Add(2, 2)).unwrap();
            assert_eq!(res, TrivialResponse::Sum(4));
            sender.send(TrivialRequest::Record(8)).unwrap();
            let res = sender.wait_on(TrivialRequest::Add(1, 1)).unwrap();
            assert_eq!(res, TrivialResponse::Sum(2));
            sender.send(TrivialRequest::Record(9)).unwrap();
        });

        service.pump();
        thread.join().unwrap();

        let record = service.into_handler().record;
        assert_eq!(&record[..], &[8, 9]);
    }

    #[test]
    #[should_panic]
    fn broken_channel() {
        let handler = TrivialHandler::default();
        let (_, sender) = Service::unbounded(handler);

        sender.send(TrivialRequest::Add(1, 1)).unwrap();
    }
}
