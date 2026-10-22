//! A container that lets the window get smaller than its contents' normal
//! minimum, for scaling them down (see `calculator::scale`).

use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use calculator::scale::MIN_SCALE;

mod imp {
    use super::*;

    pub struct ScaleBox {
        pub child: RefCell<Option<gtk::Widget>>,
        pub shrink: Cell<f64>,
        pub on_allocate: RefCell<Option<Box<dyn Fn()>>>,
    }

    impl Default for ScaleBox {
        fn default() -> Self {
            ScaleBox {
                child: RefCell::default(),
                shrink: Cell::new(MIN_SCALE),
                on_allocate: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ScaleBox {
        const NAME: &'static str = "CalcScaleBox";
        type Type = super::ScaleBox;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ScaleBox {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_overflow(gtk::Overflow::Hidden);
        }

        fn dispose(&self) {
            if let Some(child) = self.child.take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for ScaleBox {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.borrow().clone() else {
                return (0, 0, -1, -1);
            };
            let (minimum, natural, _, _) = child.measure(orientation, for_size);
            let minimum = (f64::from(minimum) * self.shrink.get()).ceil() as i32;
            (minimum, natural.max(minimum), -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.child.borrow().clone() else {
                return;
            };
            // Less room than the child needs: it keeps its minimum and is
            // clipped, rather than squeezed, until the window rescales.
            let min_w = child.measure(gtk::Orientation::Horizontal, -1).0;
            let width = width.max(min_w);
            let min_h = child.measure(gtk::Orientation::Vertical, width).0;
            child.allocate(width, height.max(min_h), baseline, None);
            if let Some(callback) = self.on_allocate.borrow().as_ref() {
                callback();
            }
        }
    }
}

glib::wrapper! {
    /// Holds one child and reports `shrink` times its minimum size, which the
    /// window sets to what the child needs at the smallest scale.
    pub struct ScaleBox(ObjectSubclass<imp::ScaleBox>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ScaleBox {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let scale_box: ScaleBox = glib::Object::new();
        child.set_parent(&scale_box);
        scale_box.imp().child.replace(Some(child.clone().upcast()));
        scale_box
    }

    pub fn child(&self) -> gtk::Widget {
        self.imp()
            .child
            .borrow()
            .clone()
            .expect("a ScaleBox has a child")
    }

    pub fn shrink(&self) -> f64 {
        self.imp().shrink.get()
    }

    pub fn set_shrink(&self, shrink: f64) {
        self.imp().shrink.set(shrink);
        self.queue_resize();
    }

    /// Called after every allocation, when sizes may have changed.
    pub fn connect_allocate(&self, callback: impl Fn() + 'static) {
        self.imp().on_allocate.replace(Some(Box::new(callback)));
    }
}
