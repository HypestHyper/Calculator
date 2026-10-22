//! Spring physics and a transform wrapper widget. Plain GTK 4, no libadwaita.
//!
//! Spring values are based on Material 3
//! motion tokens, tuned down slightly so keys settle quickly and never grow
//! past their own cell.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{graphene, gsk};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Expressive,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Fast,
    Default,
    Slow,
    Effects,
    /// Key going down: quick, no bounce.
    Press,
    /// Key coming back: a small, soft settle.
    Release,
}

thread_local! {
    /// None means no animation.
    static SCHEME: Cell<Option<Scheme>> = const { Cell::new(Some(Scheme::Expressive)) };
}

pub fn set_scheme(scheme: Option<Scheme>) {
    SCHEME.with(|s| s.set(scheme));
}

pub fn enabled() -> bool {
    SCHEME.with(Cell::get).is_some()
}

/// (stiffness, damping ratio)
fn spec(kind: Kind) -> (f64, f64) {
    match (SCHEME.with(Cell::get).unwrap_or(Scheme::Standard), kind) {
        (Scheme::Expressive, Kind::Fast) => (900.0, 0.78),
        (Scheme::Expressive, Kind::Default) => (420.0, 0.86),
        (Scheme::Expressive, Kind::Slow) => (240.0, 0.86),
        (Scheme::Expressive, Kind::Effects) => (1600.0, 1.0),
        (Scheme::Expressive, Kind::Press) => (1500.0, 1.0),
        (Scheme::Expressive, Kind::Release) => (520.0, 0.72),
        (Scheme::Standard, Kind::Fast) => (1400.0, 0.95),
        (Scheme::Standard, Kind::Default) => (700.0, 0.95),
        (Scheme::Standard, Kind::Slow) => (300.0, 0.95),
        (Scheme::Standard, Kind::Effects) => (1600.0, 1.0),
        (Scheme::Standard, Kind::Press) => (1600.0, 1.0),
        (Scheme::Standard, Kind::Release) => (900.0, 1.0),
    }
}

/// Options for `Spring::animate`.
#[derive(Default)]
pub struct Anim {
    pub kind: Option<Kind>,
    pub velocity: Option<f64>,
    pub stiffness: Option<f64>,
    pub damping: Option<f64>,
    pub done: Option<Box<dyn FnOnce()>>,
}

struct SpringState {
    value: f64,
    target: f64,
    velocity: f64,
    kind: Kind,
    k: f64,
    z: f64,
    tick: Option<gtk::TickCallbackId>,
    last: Option<f64>,
    done: Option<Box<dyn FnOnce()>>,
}

/// A damped spring driving one float. Calls `on_update(value)` each frame.
#[derive(Clone)]
pub struct Spring {
    widget: glib::WeakRef<gtk::Widget>,
    state: Rc<RefCell<SpringState>>,
    on_update: Rc<dyn Fn(f64)>,
}

impl Spring {
    pub fn new(
        widget: &gtk::Widget,
        value: f64,
        kind: Kind,
        on_update: impl Fn(f64) + 'static,
    ) -> Self {
        let (k, z) = spec(kind);
        Spring {
            widget: widget.downgrade(),
            state: Rc::new(RefCell::new(SpringState {
                value,
                target: value,
                velocity: 0.0,
                kind,
                k,
                z,
                tick: None,
                last: None,
                done: None,
            })),
            on_update: Rc::new(on_update),
        }
    }

    pub fn target(&self) -> f64 {
        self.state.borrow().target
    }

    /// Jump to `value` with no animation.
    pub fn set(&self, value: f64) {
        self.stop();
        {
            let mut s = self.state.borrow_mut();
            s.value = value;
            s.target = value;
            s.velocity = 0.0;
        }
        (self.on_update)(value);
        self.finish();
    }

    fn finish(&self) {
        let done = self.state.borrow_mut().done.take();
        if let Some(done) = done {
            done();
        }
    }

    fn stop(&self) {
        if let Some(tick) = self.state.borrow_mut().tick.take() {
            tick.remove();
        }
    }

    pub fn animate_to(&self, target: f64, kind: Kind) {
        self.animate(
            target,
            Anim {
                kind: Some(kind),
                ..Anim::default()
            },
        );
    }

    pub fn animate(&self, target: f64, anim: Anim) {
        {
            let mut s = self.state.borrow_mut();
            s.done = anim.done;
            s.target = target;
            let (k, z) = spec(anim.kind.unwrap_or(s.kind));
            s.k = anim.stiffness.unwrap_or(k);
            s.z = anim.damping.unwrap_or(z);
            if let Some(velocity) = anim.velocity {
                s.velocity = velocity;
            }
        }
        let Some(widget) = self.widget.upgrade() else {
            return;
        };
        if !enabled() || !widget.is_mapped() {
            self.set(target);
            return;
        }
        if self.state.borrow().tick.is_some() {
            return;
        }
        self.state.borrow_mut().last = None;
        let spring = self.clone();
        let tick =
            widget.add_tick_callback(move |_, clock| spring.tick(clock.frame_time() as f64 / 1e6));
        self.state.borrow_mut().tick = Some(tick);
    }

    fn tick(&self, now: f64) -> glib::ControlFlow {
        let (value, settled) = {
            let mut s = self.state.borrow_mut();
            let last = s.last.unwrap_or(now - 1.0 / 60.0);
            let elapsed = (now - last).min(0.064);
            s.last = Some(now);
            let (k, c) = (s.k, 2.0 * s.z * s.k.sqrt());
            let steps = (elapsed / (1.0 / 480.0)).ceil().max(1.0) as usize;
            let dt = elapsed / steps as f64;
            for _ in 0..steps {
                let accel = -k * (s.value - s.target) - c * s.velocity;
                s.velocity += accel * dt;
                s.value += s.velocity * dt;
            }
            let settled = (s.value - s.target).abs() < 1e-3 && s.velocity.abs() < 1e-2;
            if settled {
                s.value = s.target;
                s.velocity = 0.0;
                s.tick = None; // removed by returning Break
            }
            (s.value, settled)
        };
        (self.on_update)(value);
        if settled {
            self.finish();
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    }
}

mod imp {
    use super::*;
    use std::cell::OnceCell;

    pub struct Animated {
        pub child: RefCell<Option<gtk::Widget>>,
        pub scale_value: Cell<f64>,
        pub tx: Cell<f64>,
        pub ty: Cell<f64>,
        pub alpha: Cell<f64>,
        /// Corner radius as a fraction of half the shorter side; None = no clip.
        pub radius: Cell<Option<f64>>,
        pub springs: OnceCell<Springs>,
    }

    pub struct Springs {
        pub scale: Spring,
        pub x: Spring,
        pub y: Spring,
        pub opacity: Spring,
        pub shape: Spring,
    }

    impl Default for Animated {
        fn default() -> Self {
            Animated {
                child: RefCell::default(),
                scale_value: Cell::new(1.0),
                tx: Cell::new(0.0),
                ty: Cell::new(0.0),
                alpha: Cell::new(1.0),
                radius: Cell::new(None),
                springs: OnceCell::new(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Animated {
        const NAME: &'static str = "CalcAnimated";
        type Type = super::Animated;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Animated {
        fn dispose(&self) {
            if let Some(child) = self.child.take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Animated {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(child) = self.child.borrow().clone() else {
                return;
            };
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let (sx, tx, ty) = (
                self.scale_value.get() as f32,
                self.tx.get() as f32,
                self.ty.get() as f32,
            );
            let alpha = self.alpha.get();
            let moved = sx != 1.0 || tx != 0.0 || ty != 0.0;
            if alpha < 1.0 {
                snapshot.push_opacity(alpha);
            }
            if moved {
                snapshot.save();
                snapshot.translate(&graphene::Point::new(w / 2.0 + tx, h / 2.0 + ty));
                snapshot.scale(sx, sx);
                snapshot.translate(&graphene::Point::new(-w / 2.0, -h / 2.0));
            }
            if let Some(radius) = self.radius.get() {
                let r = radius as f32 * w.min(h) / 2.0;
                let rect = gsk::RoundedRect::from_rect(graphene::Rect::new(0.0, 0.0, w, h), r);
                snapshot.push_rounded_clip(&rect);
                widget.snapshot_child(&child, snapshot);
                snapshot.pop();
            } else {
                widget.snapshot_child(&child, snapshot);
            }
            if moved {
                snapshot.restore();
            }
            if alpha < 1.0 {
                snapshot.pop();
            }
        }
    }
}

glib::wrapper! {
    /// Wraps one child and draws it scaled, translated, faded and, optionally,
    /// clipped to a rounded shape.
    ///
    /// Everything happens at draw time, around the widget's own centre, so the
    /// layout never moves and nothing spills to one side.
    ///
    /// `shape` is the corner radius as a fraction of half the shorter side:
    /// 1.0 is a full circle or pill, 0.0 a square. Animating it gives the
    /// Material 3 Expressive shape change at any button size.
    pub struct Animated(ObjectSubclass<imp::Animated>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Animated {
    pub fn new(child: &impl IsA<gtk::Widget>, shape: Option<f64>) -> Self {
        let anim: Animated = glib::Object::new();
        let imp = anim.imp();
        imp.radius.set(shape);
        child.set_parent(&anim);
        imp.child.replace(Some(child.clone().upcast()));

        let widget = anim.upcast_ref::<gtk::Widget>();
        let weak = anim.downgrade();
        let update = move |apply: fn(&imp::Animated, f64)| {
            let weak = weak.clone();
            move |v: f64| {
                if let Some(anim) = weak.upgrade() {
                    apply(anim.imp(), v);
                    anim.queue_draw();
                }
            }
        };
        let springs = imp::Springs {
            scale: Spring::new(
                widget,
                1.0,
                Kind::Fast,
                update(|imp, v| imp.scale_value.set(v)),
            ),
            x: Spring::new(widget, 0.0, Kind::Default, update(|imp, v| imp.tx.set(v))),
            y: Spring::new(widget, 0.0, Kind::Default, update(|imp, v| imp.ty.set(v))),
            opacity: Spring::new(
                widget,
                1.0,
                Kind::Effects,
                update(|imp, v| imp.alpha.set(v.clamp(0.0, 1.0))),
            ),
            shape: Spring::new(
                widget,
                shape.unwrap_or(0.0),
                Kind::Fast,
                update(|imp, v| {
                    if imp.radius.get().is_some() {
                        imp.radius.set(Some(v.clamp(0.0, 1.0)));
                    }
                }),
            ),
        };
        let _ = imp.springs.set(springs);
        anim
    }

    fn springs(&self) -> &imp::Springs {
        self.imp().springs.get().unwrap()
    }

    pub fn scale(&self) -> &Spring {
        &self.springs().scale
    }

    pub fn x(&self) -> &Spring {
        &self.springs().x
    }

    pub fn y(&self) -> &Spring {
        &self.springs().y
    }

    pub fn opacity(&self) -> &Spring {
        &self.springs().opacity
    }

    pub fn shape(&self) -> &Spring {
        &self.springs().shape
    }

    pub fn child(&self) -> gtk::Widget {
        self.imp().child.borrow().clone().unwrap()
    }

    // Ready-made gestures -------------------------------------------------
    pub fn shake(&self) {
        self.x().set(0.0);
        let anim = Anim {
            velocity: Some(-700.0),
            stiffness: Some(900.0),
            damping: Some(0.3),
            ..Anim::default()
        };
        self.x().animate(0.0, anim);
    }

    /// Slide up into place, e.g. when a result appears.
    pub fn enter_from(&self, dy: f64, scale: f64) {
        self.y().set(dy);
        self.scale().set(scale);
        self.opacity().set(0.0);
        self.y().animate_to(0.0, Kind::Default);
        self.scale().animate_to(1.0, Kind::Default);
        self.opacity().animate_to(1.0, Kind::Effects);
    }
}

/// Press feedback for a keypad button: a slight squish plus a shape change.
///
/// rest/pressed/checked are shape fractions (see `Animated`). The scale never
/// goes above 1, so a key never covers its neighbours.
pub struct KeyMotion {
    pub anim: Animated,
    button: gtk::Widget,
    rest: f64,
    pressed: f64,
    checked: f64,
    depth: f64,
    down: Cell<bool>,
}

impl KeyMotion {
    pub fn new(
        anim: &Animated,
        button: &impl IsA<gtk::Widget>,
        rest: f64,
        pressed: f64,
    ) -> Rc<Self> {
        let motion = Rc::new(KeyMotion {
            anim: anim.clone(),
            button: button.clone().upcast(),
            rest,
            pressed,
            checked: 0.45,
            depth: 0.95,
            down: Cell::new(false),
        });
        let weak = Rc::downgrade(&motion);
        button.connect_state_flags_changed(move |widget, _| {
            if let Some(motion) = weak.upgrade() {
                motion.on_state(widget.upcast_ref());
            }
        });
        motion
    }

    fn resting_shape(&self) -> f64 {
        if self.button.state_flags().contains(gtk::StateFlags::CHECKED) {
            self.checked
        } else {
            self.rest
        }
    }

    fn on_state(&self, widget: &gtk::Widget) {
        let down = widget.state_flags().contains(gtk::StateFlags::ACTIVE);
        if down != self.down.get() {
            self.down.set(down);
            if down {
                self.anim.scale().animate_to(self.depth, Kind::Press);
                self.anim.shape().animate_to(self.pressed, Kind::Press);
            } else {
                self.anim.scale().animate_to(1.0, Kind::Release);
                self.anim
                    .shape()
                    .animate_to(self.resting_shape(), Kind::Release);
            }
        } else if !down && (self.anim.shape().target() - self.resting_shape()).abs() > 1e-3 {
            // checked changed
            self.anim
                .shape()
                .animate_to(self.resting_shape(), Kind::Default);
        }
    }

    /// A press and release, for keyboard input.
    pub fn pulse(self: &Rc<Self>) {
        self.anim.scale().animate_to(self.depth, Kind::Press);
        self.anim.shape().animate_to(self.pressed, Kind::Press);
        let motion = Rc::clone(self);
        glib::timeout_add_local_once(Duration::from_millis(110), move || {
            if !motion.down.get() {
                motion.anim.scale().animate_to(1.0, Kind::Release);
                motion
                    .anim
                    .shape()
                    .animate_to(motion.resting_shape(), Kind::Release);
            }
        });
    }
}
