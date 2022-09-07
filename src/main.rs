// Complex ThreadPools are a great way to manage the execution of tasks

use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

// The Worker struct holds the id of the worker and the thread it is running on.
// The id is used to identify the worker when it is printing out its messages.
// The thread is the actual thread that is running on the operating system.
// The JoinHandle is a type that represents the running thread.
#[derive(Debug)]
struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

// The `new` function takes an id and a receiver and returns a Worker.
// The receiver is a reference to the Arc<Mutex<mpsc::Receiver<Message>>> that
// is shared between the ThreadPool and all of its Workers.
impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        // The thread::spawn function takes a closure and returns a JoinHandle.
        // The closure moves the receiver into the closure so that it is owned by
        // the new thread. The new thread then loops forever, waiting to receive
        // messages on the channel.
        let thread = thread::spawn(move || loop {
            // The receiver.lock() call acquires the mutex. If the mutex is
            // already held by another thread, this call will block until the
            // mutex is free. The mutex is held until the end of the loop, at
            // which point the lock is dropped.
            let message = receiver.lock().unwrap().recv().unwrap();

            // The match expression is used to handle the two different kinds of
            // messages: NewJob and Terminate.
            match message {
                Message::NewJob(job) => {
                    println!("Worker {} got a job; executing.", id);

                    job();
                }
                Message::Terminate => {
                    println!("Worker {} was told to terminate.", id);

                    break;
                }
            }
        });

        Worker { id, thread }
    }
}

// The Message enum is used to send messages between the ThreadPool and its
// Workers. The NewJob variant holds a Job, which is a type alias for a Box
// containing a trait object. The trait object represents a closure that takes
// no arguments and returns nothing. The Send and 'static bounds are required
// because the ThreadPool needs to be able to send the job from one thread to
// another. The 'static bound means that the closure can outlive the current
// execution, which is required because we don't know how long the thread will
// take to execute the job. The Send bound means that the closure can be sent
// across thread boundaries, which is required because we're sending the job
// from the ThreadPool to a Worker.
enum Message {
    NewJob(Job),
    Terminate,
}
/// The Job type is a type alias for a Box containing a trait object.
type Job = Box<dyn FnOnce() + Send + 'static>;

// The ThreadPool struct holds a vector of workers and a sender.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

// The new function takes a size and returns a ThreadPool.
impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        // The mpsc::channel function returns a tuple: the first element is the
        // sending end of the channel, and the second element is the receiving
        // end. The sending end can be cloned with the mpsc::Sender::clone
        // method, which is what we do here.
        let (sender, receiver) = mpsc::channel();

        // The receiver needs to be shared between the ThreadPool and all of its
        // workers. Arc is a type that provides atomic reference counting. The
        // Arc::new function allocates some memory on the heap to store the
        // reference count. The reference count starts at one, and when the
        // ThreadPool and all of its workers go out of scope, the memory will be
        // cleaned up automatically. The Mutex is used to ensure that only one
        // Worker at a time tries to request a job from the receiver.
        let receiver = Arc::new(Mutex::new(receiver));

        // The workers vector is preallocated to the correct size. The with_capacity
        // function does not actually allocate any space for the elements, it just
        // sets the length of the vector.
        let mut workers = Vec::with_capacity(size);

        // We then iterate over the range from 0 to size, creating that number of
        // new workers and storing them in the vector.
        for id in 0..size {
            // We call the Worker::new function to create a new worker, passing
            // it the id of the worker and a clone of the receiver. The receiver
            // needs to be cloned because each Worker needs its own receiver.
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    // The execute method takes a generic F argument bounded by the FnOnce trait
    // and the Send and 'static traits. The FnOnce trait means that the closure
    // takes no arguments and returns nothing. The Send and 'static bounds mean
    // that the closure can be sent across thread boundaries and can outlive the
    // current execution, which is required because we don't know how long the
    // thread will take to execute the job.

    // The execute method takes ownership of the closure, which means that the
    // ThreadPool will take ownership of the closure and drop it when the job is
    // complete. The FnOnce trait means that the closure can be called at most
    // once, which is what we need because we're only calling it from one thread.

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // The NewJob variant of the Message enum holds a Job, which is a type
        // alias for a Box containing a trait object. The trait object represents
        // a closure that takes no arguments and returns nothing. The Send and
        // 'static bounds are required because the ThreadPool needs to be able to
        // send the job from one thread to another. The 'static bound means that
        // the closure can outlive the current execution, which is required
        // because we don't know how long the thread will take to execute the
        // job. The Send bound means that the closure can be sent across thread
        // boundaries, which is required because we're sending the job from the
        // ThreadPool to a Worker.
        let job = Box::new(f);

        // The send method on the sender will send the message down the channel
        // and return a Result. The Result will have the Err variant if the
        // receiving end of the channel has already been dropped, which would
        // happen if the ThreadPool were dropped. If the receiving end of the
        // channel is still open, the send method will transfer ownership of the
        // job to the receiving end and return the Ok variant containing the
        // number of messages in the channel before the send. We don't do
        // anything with the number of messages in the channel, so we use the
        // _ pattern to ignore it.
        self.sender.send(Message::NewJob(job)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        println!("Sending terminate message to all workers.");

        // The send method on the sender will send the message down the channel
        // and return a Result. The Result will have the Err variant if the
        // receiving end of the channel has already been dropped, which would
        // happen if the ThreadPool were dropped. If the receiving end of the
        // channel is still open, the send method will transfer ownership of the
        // job to the receiving end and return the Ok variant containing the
        // number of messages in the channel before the send. We don't do
        // anything with the number of messages in the channel, so we use the
        // _ pattern to ignore it.
        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        println!("Shutting down all workers.");

        // We then loop over the rest of the workers, popping them off the
        // end of the vector and calling join on their threads. If the join
        // method returns an error, we panic. If the join method returns
        // successfully, we call the thread method on the JoinHandle to get
        // the underlying thread.
        while let Some(worker) = self.workers.pop() {
            println!("Shutting down worker {}", worker.id);

            if let Ok(thread) = worker.thread.join() {
                thread
            }

            println!("Worker {} shut down.", worker.id);
        }
    }
}

fn main() {
    let pool = ThreadPool::new(4);

    (0..8).for_each(|i| {
        pool.execute(move || {
            println!("Hello from worker {}", i);
        });
    });
}
