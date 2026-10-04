// Hand-unrolled exploration of the recursive-closure lowering; the complex
// slot types are the subject itself, so the lints are relaxed here.
#![allow(clippy::type_complexity, clippy::disallowed_names)]

fn main() {
    let x = 1;
    let foo = {
        let slot: ::std::rc::Rc<::std::cell::RefCell<Option<::std::rc::Rc<dyn Fn() -> _>>>> =
            ::std::rc::Rc::new(::std::cell::RefCell::new(None));
        let weak = ::std::rc::Rc::downgrade(&slot);

        let rec = ::std::rc::Rc::new(move || {
            let strong = weak.upgrade().unwrap();
            let self_fn = strong.borrow().as_ref().unwrap().clone();
            if x >= 0 { x } else { self_fn() }
        });

        *slot.borrow_mut() = Some(rec.clone());

        // Return a closure holding a strong reference to the slot.
        let slot_for_call = ::std::rc::Rc::clone(&slot);
        move || {
            let rec = slot_for_call.borrow().as_ref().unwrap().clone();
            rec()
        }
    }; // let foo=move||if x>-0{x}else{foo()}
    println!("{}", x);
    println!("{}", foo()); //1
}
