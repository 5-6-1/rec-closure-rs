// Hand-unrolled exploration of the recursive-closure lowering; the complex
// slot types are the subject itself, so the lint is relaxed here.
#![allow(clippy::type_complexity)]

fn main() {
    let x = 1;
    let gcd = {
        let slot: ::std::rc::Rc<::std::cell::RefCell<Option<::std::rc::Rc<dyn Fn(_, _) -> _>>>> =
            ::std::rc::Rc::new(::std::cell::RefCell::new(None));
        let weak = ::std::rc::Rc::downgrade(&slot);

        let rec = ::std::rc::Rc::new(move |mut a, mut b| {
            let strong = weak.upgrade().unwrap();
            let self_fn = strong.borrow().as_ref().unwrap().clone();
            if a < b {
                [a, b] = [b, a]
            }
            if b != 0 { self_fn(b, a % b) } else { a }
        });

        *slot.borrow_mut() = Some(rec.clone());

        // Return a closure holding a strong reference to the slot.
        let slot_for_call = ::std::rc::Rc::clone(&slot);
        move |a, b| {
            let rec = slot_for_call.borrow().as_ref().unwrap().clone();
            rec(a, b)
        }
    }; // let gcd=move|mut a,mut b|{if a<b{[a,b]=[b,a]}if b!=0{gcd(b,a%b)}else{a}};
    println!("{}", x);
    println!("{}", gcd(12, 32)); // 4
}
