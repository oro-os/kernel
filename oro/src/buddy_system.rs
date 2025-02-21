//! Buddy system allocator implementation.
//!
//! This is an excerpt from the `buddy_system_allocator` crate,
//! adapted for use in Oro modules.
//!
//! Copyright 2019-2020 Jiajie Chen, licensed under the MIT License.
//! Original: <https://github.com/rcore-os/buddy_system_allocator>
#![expect(
	 // NOTE(qix-): I intend to audit this at some point; this is temporary.
	unsafe_op_in_unsafe_fn,
	clippy::missing_docs_in_private_items,
)]

use core::{
	alloc::Layout,
	cmp::{max, min},
	mem::size_of,
	ptr::NonNull,
};

/// A heap that uses buddy system with configurable order.
pub struct Heap<const ORDER: usize> {
	// buddy system with max order of `ORDER - 1`
	free_list: [linked_list::LinkedList; ORDER],

	// statistics
	user:      usize,
	allocated: usize,
	total:     usize,
}

impl<const ORDER: usize> Heap<ORDER> {
	/// Create an empty heap
	pub const fn new() -> Self {
		Heap {
			free_list: [linked_list::LinkedList::new(); ORDER],
			user:      0,
			allocated: 0,
			total:     0,
		}
	}

	/// Add a range of memory [start, end) to the heap
	pub unsafe fn add_to_heap(&mut self, mut start: usize, mut end: usize) {
		// avoid unaligned access on some platforms
		start = (start + size_of::<usize>() - 1) & (!size_of::<usize>() + 1);
		end &= !size_of::<usize>() + 1;
		assert!(start <= end);

		let mut total = 0;
		let mut current_start = start;

		while current_start + size_of::<usize>() <= end {
			let lowbit = current_start & (!current_start + 1);
			let mut size = min(lowbit, prev_power_of_two(end - current_start));

			// If the order of size is larger than the max order,
			// split it into smaller blocks.
			let mut order = size.trailing_zeros() as usize;
			if order > ORDER - 1 {
				order = ORDER - 1;
				size = 1 << order;
			}
			total += size;

			self.free_list[order].push(current_start as *mut usize);
			current_start += size;
		}

		self.total += total;
	}

	/// Alloc a range of memory from the heap satifying `layout` requirements
	pub fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
		let size = max(
			layout.size().next_power_of_two(),
			max(layout.align(), size_of::<usize>()),
		);
		let class = size.trailing_zeros() as usize;
		for i in class..self.free_list.len() {
			// Find the first non-empty size class
			if !self.free_list[i].is_empty() {
				// Split buffers
				for j in ((class + 1)..=i).rev() {
					if let Some(block) = self.free_list[j].pop() {
						unsafe {
							self.free_list[j - 1]
								.push((block as usize + (1 << (j - 1))) as *mut usize);
							self.free_list[j - 1].push(block);
						}
					} else {
						return Err(());
					}
				}

				let Some(result) = NonNull::new(
					self.free_list[class]
						.pop()
						.expect("current block should have free space now")
						.cast::<u8>(),
				) else {
					return Err(());
				};

				self.user += layout.size();
				self.allocated += size;
				return Ok(result);
			}
		}
		Err(())
	}

	/// Dealloc a range of memory from the heap
	///
	/// # Safety
	/// `ptr` must be a pointer to a memory block previously allocated by this heap,
	/// must not be dangling, and must not be null. It must not have been previously
	/// deallocated.
	// NOTE(qix-): Marked as `unsafe` in response to
	// NOTE(qix-): <https://github.com/rcore-os/buddy_system_allocator/issues/37>
	pub unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
		let size = max(
			layout.size().next_power_of_two(),
			max(layout.align(), size_of::<usize>()),
		);
		let class = size.trailing_zeros() as usize;

		unsafe {
			// Put back into free list
			#[expect(clippy::cast_ptr_alignment)]
			self.free_list[class].push(ptr.as_ptr().cast::<usize>());

			// Merge free buddy lists
			let mut current_ptr = ptr.as_ptr() as usize;
			let mut current_class = class;

			while current_class < self.free_list.len() - 1 {
				let buddy = current_ptr ^ (1 << current_class);
				let mut flag = false;
				for block in self.free_list[current_class].iter_mut() {
					if block.value() as usize == buddy {
						block.pop();
						flag = true;
						break;
					}
				}

				// Free buddy found
				if flag {
					self.free_list[current_class].pop();
					current_ptr = min(current_ptr, buddy);
					current_class += 1;
					self.free_list[current_class].push(current_ptr as *mut usize);
				} else {
					break;
				}
			}
		}

		self.user -= layout.size();
		self.allocated -= size;
	}

	/// Return the number of bytes that user requests
	#[expect(dead_code)] // TODO(qix-): May be useful; will keep for now.
	pub fn stats_alloc_user(&self) -> usize {
		self.user
	}

	/// Return the number of bytes that are actually allocated
	#[expect(dead_code)] // TODO(qix-): May be useful; will keep for now.
	pub fn stats_alloc_actual(&self) -> usize {
		self.allocated
	}

	/// Return the total number of bytes in the heap
	#[expect(dead_code)] // TODO(qix-): May be useful; will keep for now.
	pub fn stats_total_bytes(&self) -> usize {
		self.total
	}
}

fn prev_power_of_two(num: usize) -> usize {
	1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}

mod linked_list {
	use core::{marker::PhantomData, ptr};

	/// An intrusive linked list
	///
	/// A clean room implementation of the one used in CS140e 2018 Winter
	///
	/// Thanks Sergio Benitez for his excellent work,
	/// See [CS140e](https://cs140e.sergio.bz/) for more information
	#[derive(Copy, Clone)]
	pub struct LinkedList {
		head: *mut usize,
	}

	unsafe impl Send for LinkedList {}

	impl LinkedList {
		/// Create a new [`LinkedList`]
		pub const fn new() -> Self {
			Self {
				head: ptr::null_mut(),
			}
		}

		/// Return `true` if the list is empty
		pub fn is_empty(&self) -> bool {
			self.head.is_null()
		}

		/// Push `item` to the front of the list
		pub unsafe fn push(&mut self, item: *mut usize) {
			*item = self.head as usize;
			self.head = item;
		}

		/// Try to remove the first item in the list
		pub fn pop(&mut self) -> Option<*mut usize> {
			if self.is_empty() {
				None
			} else {
				// Advance head pointer
				let item = self.head;
				self.head = unsafe { *item as *mut usize };
				Some(item)
			}
		}

		/// Return an iterator over the items in the list
		#[expect(dead_code)] // TODO(qix-): May be useful; will keep for now.
		pub fn iter(&self) -> Iter<'_> {
			Iter {
				curr: self.head,
				list: PhantomData,
			}
		}

		/// Return an mutable iterator over the items in the list
		pub fn iter_mut(&mut self) -> IterMut<'_> {
			IterMut {
				prev: (&raw mut self.head).cast::<usize>(),
				curr: self.head,
				list: PhantomData,
			}
		}
	}

	/// An iterator over the linked list
	pub struct Iter<'a> {
		curr: *mut usize,
		list: PhantomData<&'a LinkedList>,
	}

	impl Iterator for Iter<'_> {
		type Item = *mut usize;

		fn next(&mut self) -> Option<Self::Item> {
			if self.curr.is_null() {
				None
			} else {
				let item = self.curr;
				let next = unsafe { *item as *mut usize };
				self.curr = next;
				Some(item)
			}
		}
	}

	/// Represent a mutable node in `LinkedList`
	pub struct ListNode {
		prev: *mut usize,
		curr: *mut usize,
	}

	impl ListNode {
		/// Remove the node from the list
		pub fn pop(self) -> *mut usize {
			// Skip the current one
			unsafe {
				*(self.prev) = *(self.curr);
			}
			self.curr
		}

		/// Returns the pointed address
		pub fn value(&self) -> *mut usize {
			self.curr
		}
	}

	/// A mutable iterator over the linked list
	pub struct IterMut<'a> {
		list: PhantomData<&'a mut LinkedList>,
		prev: *mut usize,
		curr: *mut usize,
	}

	impl Iterator for IterMut<'_> {
		type Item = ListNode;

		fn next(&mut self) -> Option<Self::Item> {
			if self.curr.is_null() {
				None
			} else {
				let res = ListNode {
					prev: self.prev,
					curr: self.curr,
				};
				self.prev = self.curr;
				self.curr = unsafe { *self.curr as *mut usize };
				Some(res)
			}
		}
	}
}
