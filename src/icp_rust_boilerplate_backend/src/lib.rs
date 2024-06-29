#[macro_use]
extern crate serde; // Import the serde library for serialization and deserialization

use candid::{Decode, Encode}; // Import Decode and Encode from the candid library
use ic_cdk::api::time; // Import the time API from ic_cdk
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory}; // Import memory management structures from ic_stable_structures
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable}; // Import stable structures
use std::{borrow::Cow, cell::RefCell}; // Import Cow and RefCell from the standard library

type Memory = VirtualMemory<DefaultMemoryImpl>; // Type alias for VirtualMemory using DefaultMemoryImpl
type IdCell = Cell<u64, Memory>; // Type alias for Cell storing u64 with Memory

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)] // Derive macros for the Book struct
struct Book {
    id: u64, // Unique identifier for the book
    title: String, // Title of the book
    author: String, // Author of the book
    created_at: u64, // Timestamp of when the book was created
    updated_at: Option<u64>, // Optional timestamp of when the book was last updated
}

// Implement the Storable trait for the Book struct
impl Storable for Book {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap()) // Serialize the Book struct to bytes
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap() // Deserialize bytes to a Book struct
    }
}

// Implement the BoundedStorable trait for the Book struct
impl BoundedStorable for Book {
    const MAX_SIZE: u32 = 1024; // Maximum size of the serialized Book in bytes
    const IS_FIXED_SIZE: bool = false; // Indicates that the size is not fixed
}

thread_local! {
    // Thread-local storage for memory manager
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    // Thread-local storage for ID counter
    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    // Thread-local storage for the book storage
    static STORAGE: RefCell<StableBTreeMap<u64, Book, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)] // Derive macros for the BookPayload struct
struct BookPayload {
    title: String, // Title of the book
    author: String, // Author of the book
}

#[ic_cdk::query] // Mark the function as a query method
fn get_book(id: u64) -> Result<Book, Error> {
    match _get_book(&id) {
        Some(book) => Ok(book), // Return the book if found
        None => Err(Error::NotFound {
            msg: format!("a book with id={} not found", id), // Return an error if the book is not found
        }),
    }
}

#[ic_cdk::update] // Mark the function as an update method
fn add_book(book: BookPayload) -> Result<Book, Error>  {
    if book.title.is_empty() || book.author.is_empty() {
        return Err(Error::InvalidInput { msg: "All fields must be provided and non-empty".to_string() });
    }

    // Increment the ID counter
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");

    // Create a new Book struct
    let book = Book {
        id,
        title: book.title,
        author: book.author,
        created_at: time(),
        updated_at: None,
    };

    // Insert the new book into storage
    do_insert(&book);

    Ok(book)
}

#[ic_cdk::update] // Mark the function as an update method
fn update_book(id: u64, payload: BookPayload) -> Result<Book, Error> {
    if payload.title.is_empty() || payload.author.is_empty() {
        return Err(Error::InvalidInput { msg: "All fields must be provided and non-empty".to_string() });
    }

    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut book) => {
            book.title = payload.title;
            book.author = payload.author;
            book.updated_at = Some(time());

            // Update the book in storage
            do_insert(&book);

            Ok(book)
        }
        None => Err(Error::NotFound {
            msg: format!("couldn't update a book with id={}. book not found", id),
        }),
    }
}

// Helper method to perform insert operation
fn do_insert(book: &Book) {
    STORAGE.with(|service| service.borrow_mut().insert(book.id, book.clone()));
}

#[ic_cdk::update] // Mark the function as an update method
fn delete_book(id: u64) -> Result<Book, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(book) => Ok(book), // Return the deleted book if found
        None => Err(Error::NotFound {
            msg: format!("couldn't delete a book with id={}. book not found.", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)] // Derive macros for the Error enum
enum Error {
    NotFound { msg: String }, // Error variant for not found
    InvalidInput { msg: String }, // Error variant for invalid input
}

// Helper method to get a book by ID, used in get_book and update_book
fn _get_book(id: &u64) -> Option<Book> {
    STORAGE.with(|service| service.borrow().get(id))
}

// Generate candid interface
ic_cdk::export_candid!();
