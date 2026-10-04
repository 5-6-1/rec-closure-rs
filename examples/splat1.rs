// Hand-unrolled exploration of the recursive-closure lowering; the complex
// slot types are the subject itself, so the lint is relaxed here.
#![allow(clippy::type_complexity)]

fn main() {
    let x = 1;
    let fact = {
        let slot: ::std::rc::Rc<::std::cell::RefCell<Option<::std::rc::Rc<dyn Fn(_) -> _>>>> =
            ::std::rc::Rc::new(::std::cell::RefCell::new(None));
        let weak = ::std::rc::Rc::downgrade(&slot);

        let rec = ::std::rc::Rc::new(move |n| {
            let strong = weak.upgrade().unwrap();
            let self_fn = strong.borrow().as_ref().unwrap().clone();
            if n <= 1 { x } else { n * self_fn(n - 1) }
        });

        *slot.borrow_mut() = Some(rec.clone());

        // Return a closure holding a strong reference to the slot.
        let slot_for_call = ::std::rc::Rc::clone(&slot);
        move |n| {
            let rec = slot_for_call.borrow().as_ref().unwrap().clone();
            rec(n)
        }
    }; // let fact=move|n|if n<=1{x}else{n*fact(n-1)};
    println!("{}", x);
    println!("{}", fact(5)); // 120
}
