"""Spring physics and a transform wrapper widget. Plain GTK4, no libadwaita.

Spring values are based on Material 3 motion tokens, tuned down slightly so
keys settle quickly and never grow past their own cell.
"""

import math

import gi

gi.require_version("Graphene", "1.0")
gi.require_version("Gsk", "4.0")
from gi.repository import GLib, Graphene, Gsk, Gtk  # noqa: E402

# name -> (stiffness, damping ratio)
SCHEMES = {
    "expressive": {
        "fast": (900.0, 0.78),
        "default": (420.0, 0.86),
        "slow": (240.0, 0.86),
        "effects": (1600.0, 1.0),
        "press": (1500.0, 1.0),     # key going down: quick, no bounce
        "release": (520.0, 0.72),  # key coming back: a small, soft settle
    },
    "standard": {
        "fast": (1400.0, 0.95),
        "default": (700.0, 0.95),
        "slow": (300.0, 0.95),
        "effects": (1600.0, 1.0),
        "press": (1600.0, 1.0),
        "release": (900.0, 1.0),
    },
}


class _Motion:
    scheme = "expressive"  # or "standard", or None for no animation

    def spec(self, kind):
        return SCHEMES[self.scheme or "standard"][kind]

    @property
    def enabled(self):
        return self.scheme is not None


motion = _Motion()


class Spring:
    """A damped spring driving one float. Calls on_update(value) each frame."""

    def __init__(self, widget, value, on_update, kind="default"):
        self.widget = widget
        self.value = float(value)
        self.target = float(value)
        self.velocity = 0.0
        self.kind = kind
        self.on_update = on_update
        self._tick_id = 0
        self._last = None
        self._done = None
        self._k, self._z = motion.spec(kind)

    def set(self, value):
        self._stop()
        self.value = self.target = float(value)
        self.velocity = 0.0
        self.on_update(self.value)
        self._finish()

    def _finish(self):
        done, self._done = self._done, None
        if done is not None:
            done()

    def animate_to(self, target, kind=None, velocity=None, stiffness=None, damping=None, done=None):
        self._done = done
        self.target = float(target)
        k, z = motion.spec(kind or self.kind)
        self._k = stiffness if stiffness is not None else k
        self._z = damping if damping is not None else z
        if velocity is not None:
            self.velocity = float(velocity)
        if not motion.enabled or not self.widget.get_mapped():
            self.set(target)
            return
        if not self._tick_id:
            self._last = None
            self._tick_id = self.widget.add_tick_callback(self._tick)

    def _stop(self):
        if self._tick_id:
            self.widget.remove_tick_callback(self._tick_id)
            self._tick_id = 0

    def _tick(self, _widget, clock):
        now = clock.get_frame_time() / 1e6
        if self._last is None:
            self._last = now - 1 / 60
        elapsed = min(now - self._last, 0.064)
        self._last = now
        k, c = self._k, 2.0 * self._z * math.sqrt(self._k)
        steps = max(1, math.ceil(elapsed / (1 / 480)))
        dt = elapsed / steps
        for _ in range(steps):
            accel = -k * (self.value - self.target) - c * self.velocity
            self.velocity += accel * dt
            self.value += self.velocity * dt
        if abs(self.value - self.target) < 1e-3 and abs(self.velocity) < 1e-2:
            self.value = self.target
            self.velocity = 0.0
            self._tick_id = 0
            self.on_update(self.value)
            self._finish()
            return False
        self.on_update(self.value)
        return True


class Animated(Gtk.Widget):
    """Wraps one child and draws it scaled, translated, faded and,
    optionally, clipped to a rounded shape.

    Everything happens at draw time, around the widget's own centre, so the
    layout never moves and nothing spills to one side.

    `shape` is the corner radius as a fraction of half the shorter side:
    1.0 is a full circle or pill, 0.0 a square. Animating it gives the
    Material 3 Expressive shape change at any button size.
    """

    __gtype_name__ = "CalcAnimated"

    def __init__(self, child=None, shape=None, **kwargs):
        super().__init__(**kwargs)
        self.set_layout_manager(Gtk.BinLayout())
        self._child = None
        self.sx = self.sy = 1.0
        self.tx = self.ty = 0.0
        self.alpha = 1.0
        self.radius = shape  # None = no clipping
        self.scale = Spring(self, 1.0, self._set_scale, "fast")
        self.x = Spring(self, 0.0, self._set_tx, "default")
        self.y = Spring(self, 0.0, self._set_ty, "default")
        self.opacity = Spring(self, 1.0, self._set_alpha, "effects")
        self.shape = Spring(self, shape or 0.0, self._set_shape, "fast")
        if child is not None:
            self.set_child(child)

    def set_child(self, child):
        if self._child is not None:
            self._child.unparent()
        self._child = child
        child.set_parent(self)

    def get_child(self):
        return self._child

    def do_dispose(self):
        if self._child is not None:
            self._child.unparent()
            self._child = None
        Gtk.Widget.do_dispose(self)

    def _set_scale(self, v):
        self.sx = self.sy = v
        self.queue_draw()

    def _set_tx(self, v):
        self.tx = v
        self.queue_draw()

    def _set_ty(self, v):
        self.ty = v
        self.queue_draw()

    def _set_alpha(self, v):
        self.alpha = max(0.0, min(1.0, v))
        self.queue_draw()

    def _set_shape(self, v):
        if self.radius is not None:
            self.radius = max(0.0, min(1.0, v))
            self.queue_draw()

    def do_snapshot(self, snapshot):
        child = self._child
        if child is None:
            return
        w, h = self.get_width(), self.get_height()
        moved = self.sx != 1.0 or self.tx != 0.0 or self.ty != 0.0
        if self.alpha < 1.0:
            snapshot.push_opacity(self.alpha)
        if moved:
            snapshot.save()
            snapshot.translate(Graphene.Point().init(w / 2.0 + self.tx, h / 2.0 + self.ty))
            snapshot.scale(self.sx, self.sy)
            snapshot.translate(Graphene.Point().init(-w / 2.0, -h / 2.0))
        if self.radius is not None:
            r = self.radius * min(w, h) / 2.0
            rect = Gsk.RoundedRect()
            rect.init_from_rect(Graphene.Rect().init(0, 0, w, h), r)
            snapshot.push_rounded_clip(rect)
            self.snapshot_child(child, snapshot)
            snapshot.pop()
        else:
            self.snapshot_child(child, snapshot)
        if moved:
            snapshot.restore()
        if self.alpha < 1.0:
            snapshot.pop()

    # Ready-made gestures -------------------------------------------------
    def shake(self):
        self.x.set(0.0)
        self.x.animate_to(0.0, velocity=-700.0, stiffness=900.0, damping=0.3)

    def enter_from(self, dy=24.0, scale=0.96):
        """Slide up into place, e.g. when a result appears."""
        self.y.set(dy)
        self.scale.set(scale)
        self.opacity.set(0.0)
        self.y.animate_to(0.0, "default")
        self.scale.animate_to(1.0, "default")
        self.opacity.animate_to(1.0, "effects")


class KeyMotion:
    """Press feedback for a keypad button: a slight squish plus a shape change.

    rest/pressed/checked are shape fractions (see Animated). The scale never
    goes above 1, so a key never covers its neighbours.
    """

    def __init__(self, anim, button, rest=1.0, pressed=0.55, checked=0.45, depth=0.95):
        self.anim, self.button = anim, button
        self.rest, self.pressed, self.checked, self.depth = rest, pressed, checked, depth
        self.down = False
        button.connect("state-flags-changed", self._on_state)

    def _resting_shape(self):
        if self.button.get_state_flags() & Gtk.StateFlags.CHECKED:
            return self.checked
        return self.rest

    def _on_state(self, widget, _old):
        flags = widget.get_state_flags()
        down = bool(flags & Gtk.StateFlags.ACTIVE)
        if down != self.down:
            self.down = down
            if down:
                self.anim.scale.animate_to(self.depth, "press")
                self.anim.shape.animate_to(self.pressed, "press")
            else:
                self.anim.scale.animate_to(1.0, "release")
                self.anim.shape.animate_to(self._resting_shape(), "release")
        elif not down and abs(self.anim.shape.target - self._resting_shape()) > 1e-3:
            self.anim.shape.animate_to(self._resting_shape(), "default")  # checked changed

    def pulse(self):
        """A press and release, for keyboard input."""
        self.anim.scale.animate_to(self.depth, "press")
        self.anim.shape.animate_to(self.pressed, "press")

        def back():
            if not self.down:
                self.anim.scale.animate_to(1.0, "release")
                self.anim.shape.animate_to(self._resting_shape(), "release")
            return GLib.SOURCE_REMOVE
        GLib.timeout_add(110, back)
