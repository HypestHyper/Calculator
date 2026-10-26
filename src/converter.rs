//! The unit converter shown in place of the calculator's display: a
//! category, the value typed on the keypad in one unit, and the same value
//! in another. Conversions are in `calculator::units`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use calculator::engine::{format_number, localize, Format};
use calculator::logic::{Calculator, Status};
use calculator::units::{self, Category, Unit};
use gtk::pango;
use gtk::prelude::*;

use crate::config::ConverterState;
use crate::icons::icon;

/// Significant digits in a result: enough for any everyday conversion,
/// short enough to fit.
const RESULT_DIGITS: usize = 10;

pub struct Converter {
    pub root: gtk::Box,
    category_button: gtk::MenuButton,
    category_label: gtk::Label,
    from_button: gtk::MenuButton,
    from_label: gtk::Label,
    to_button: gtk::MenuButton,
    to_label: gtk::Label,
    from_value: gtk::Label,
    to_value: gtk::Label,
    calc: RefCell<Calculator>,
    state: RefCell<ConverterState>,
    category: Cell<&'static Category>,
    from: Cell<&'static Unit>,
    to: Cell<&'static Unit>,
    /// Number format: decimals (None is automatic) and digit grouping.
    format: Cell<(Option<usize>, bool)>,
    /// The values' text size in px before fitting, and as fitted.
    base_size: Cell<f64>,
    from_size: Cell<f64>,
    to_size: Cell<f64>,
}

/// A compact button showing a choice, with a list to pick from.
fn picker(css: &str) -> (gtk::MenuButton, gtk::Label) {
    let label = gtk::Label::new(None);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    content.append(&label);
    let arrow = icon("expand_more", 16);
    arrow.add_css_class("picker-arrow");
    content.append(&arrow);
    let button = gtk::MenuButton::builder()
        .child(&content)
        .valign(gtk::Align::Center)
        .css_classes(["picker", css])
        .build();
    button.set_focus_on_click(false);
    (button, label)
}

/// Fill `button`'s list with `items`; picking one calls `chosen` with its
/// index.
fn set_choices(
    button: &gtk::MenuButton,
    items: &[String],
    current: usize,
    chosen: impl Fn(usize) + 'static,
) {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["picker-list"])
        .build();
    for (i, item) in items.iter().enumerate() {
        let label = gtk::Label::builder().label(item).xalign(0.0).build();
        if i == current {
            label.add_css_class("current");
        }
        list.append(&label);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(320)
        .child(&list)
        .build();
    let popover = gtk::Popover::builder().child(&scroll).build();
    popover.add_css_class("picker-popover");
    let weak = popover.downgrade();
    list.connect_row_activated(move |_, row| {
        if let Some(popover) = weak.upgrade() {
            popover.popdown();
        }
        chosen(row.index() as usize);
    });
    button.set_popover(Some(&popover));
}

impl Converter {
    pub fn new() -> Rc<Self> {
        let state = ConverterState::load_default();
        let category = units::category(&state.category).unwrap_or(&units::categories()[0]);

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .spacing(2)
            .css_classes(["display", "converter"])
            .build();

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let (category_button, category_label) = picker("category");
        top.append(&category_button);
        top.append(&gtk::Box::builder().hexpand(true).build());
        let swap = gtk::Button::builder()
            .child(&icon("swap_vert", 20))
            .tooltip_text("Swap units")
            .valign(gtk::Align::Center)
            .css_classes(["icon-btn", "swap"])
            .build();
        swap.set_focus_on_click(false);
        top.append(&swap);
        root.append(&top);

        let row = |value_css: &str| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let (button, label) = picker("unit");
            row.append(&button);
            let value = gtk::Label::builder()
                .xalign(1.0)
                .hexpand(true)
                .ellipsize(pango::EllipsizeMode::Start)
                .css_classes(["conv-value", value_css])
                .build();
            row.append(&value);
            root.append(&row);
            (button, label, value)
        };
        let (from_button, from_label, from_value) = row("input");
        let (to_button, to_label, to_value) = row("output");

        let converter = Rc::new(Converter {
            root,
            category_button,
            category_label,
            from_button,
            from_label,
            to_button,
            to_label,
            from_value,
            to_value,
            calc: RefCell::new(Calculator::new()),
            state: RefCell::new(state),
            category: Cell::new(category),
            from: Cell::new(&category.units[0]),
            to: Cell::new(&category.units[0]),
            format: Cell::new((None, true)),
            base_size: Cell::new(40.0),
            from_size: Cell::new(0.0),
            to_size: Cell::new(0.0),
        });

        let weak = Rc::downgrade(&converter);
        swap.connect_clicked(move |_| {
            if let Some(converter) = weak.upgrade() {
                converter.swap();
            }
        });
        converter.set_category(category);
        converter
    }

    pub fn is_open(&self) -> bool {
        self.state.borrow().open
    }

    pub fn set_open(&self, open: bool) {
        let mut state = self.state.borrow_mut();
        if state.open != open {
            state.open = open;
            state.save();
        }
    }

    fn set_category(self: &Rc<Self>, category: &'static Category) {
        self.category.set(category);
        let (from, to) = {
            let state = self.state.borrow();
            let saved = state.units.get(category.key);
            let pick = |key: Option<&String>, default: &str| {
                key.and_then(|k| category.unit(k))
                    .or_else(|| category.unit(default))
                    .unwrap_or(&category.units[0])
            };
            (
                pick(saved.map(|s| &s.0), category.default_pair.0),
                pick(saved.map(|s| &s.1), category.default_pair.1),
            )
        };
        self.category_label.set_label(category.name);
        let names: Vec<String> = units::categories()
            .iter()
            .map(|c| c.name.to_string())
            .collect();
        let current = units::categories()
            .iter()
            .position(|c| c.key == category.key)
            .unwrap_or(0);
        let weak = Rc::downgrade(self);
        set_choices(&self.category_button, &names, current, move |i| {
            if let Some(converter) = weak.upgrade() {
                converter.set_category(&units::categories()[i]);
            }
        });
        self.set_units(from, to);
    }

    fn set_units(self: &Rc<Self>, from: &'static Unit, to: &'static Unit) {
        self.from.set(from);
        self.to.set(to);
        self.from_label.set_label(from.symbol);
        self.to_label.set_label(to.symbol);
        self.from_button.set_tooltip_text(Some(from.name));
        self.to_button.set_tooltip_text(Some(to.name));
        let category = self.category.get();
        let names: Vec<String> = category
            .units
            .iter()
            .map(|u| format!("{} ({})", u.name, u.symbol))
            .collect();
        let index = |unit: &Unit| {
            category
                .units
                .iter()
                .position(|u| u.key == unit.key)
                .unwrap_or(0)
        };
        let weak = Rc::downgrade(self);
        set_choices(&self.from_button, &names, index(from), move |i| {
            if let Some(c) = weak.upgrade() {
                c.set_units(&c.category.get().units[i], c.to.get());
            }
        });
        let weak = Rc::downgrade(self);
        set_choices(&self.to_button, &names, index(to), move |i| {
            if let Some(c) = weak.upgrade() {
                c.set_units(c.from.get(), &c.category.get().units[i]);
            }
        });
        {
            let mut state = self.state.borrow_mut();
            state.category = category.key.to_string();
            state.units.insert(
                category.key.to_string(),
                (from.key.to_string(), to.key.to_string()),
            );
            state.save();
        }
        self.refresh();
    }

    /// Swap the units, with the result as the new value, so the two lines
    /// just change places.
    fn swap(self: &Rc<Self>) {
        let result = self.result();
        if let Some(value) = result {
            let mut calc = self.calc.borrow_mut();
            let decimal = calc.decimal;
            *calc = Calculator::new();
            calc.decimal = decimal;
            calc.use_value(value);
        }
        self.set_units(self.to.get(), self.from.get());
    }

    /// For development: open the list of "from" units.
    pub fn dev_open_units(&self) {
        self.from_button.popup();
    }

    /// For development: switch to the category with this key.
    pub fn dev_category(self: &Rc<Self>, key: &str) {
        if let Some(category) = units::category(key) {
            self.set_category(category);
        }
    }

    /// The typed value: the expression's value, or 0 when empty.
    fn value(&self) -> Option<f64> {
        let calc = self.calc.borrow();
        if calc.evaluated {
            Some(calc.ans)
        } else if calc.expr.is_empty() {
            Some(0.0)
        } else {
            calc.evaluate(&calc.expr).ok()
        }
    }

    fn result(&self) -> Option<f64> {
        units::convert(self.value()?, self.from.get(), self.to.get())
    }

    /// Apply a keypad key to the value.
    pub fn press(self: &Rc<Self>, action: &str) -> Status {
        let status = self.calc.borrow_mut().press(action);
        self.refresh();
        status
    }

    pub fn paste(self: &Rc<Self>, text: &str) {
        if self.calc.borrow_mut().paste(text) == Status::Ok {
            self.refresh();
        }
    }

    /// The result as text to copy, without grouping.
    pub fn copy_text(&self) -> Option<String> {
        let format = Format {
            decimals: self.format.get().0,
            grouping: false,
            pretty_minus: false,
            ..Format::default()
        };
        let decimal = self.calc.borrow().decimal;
        self.result()
            .map(|v| localize(&format_number(v, &format), decimal))
    }

    /// The values' text size before fitting, in px.
    pub fn set_base_size(&self, px: f64) {
        self.base_size.set(px);
        self.fit_values(px);
    }

    /// Number format and decimal symbol, from the settings.
    pub fn set_format(&self, decimals: Option<usize>, grouping: bool, decimal: char) {
        self.format.set((decimals, grouping));
        self.calc.borrow_mut().decimal = decimal;
        self.refresh();
    }

    fn refresh(&self) {
        let (decimals, grouping) = self.format.get();
        let decimal = self.calc.borrow().decimal;
        self.from_value
            .set_label(&self.calc.borrow().display(decimals, grouping));
        let result = self.result().map(|v| {
            let format = Format {
                decimals,
                grouping,
                max_digits: RESULT_DIGITS,
                ..Format::default()
            };
            localize(&format_number(v, &format), decimal)
        });
        self.to_value.set_label(result.as_deref().unwrap_or("—"));
        self.fit_values(self.base_size.get());
    }

    /// Shrink the values' text to fit the width, from `base` px down to
    /// 55% of it, like the calculator's display; past that it is cut at the
    /// start. Measured on a copy of each label's layout, so the label only
    /// changes when the size does: this runs on every layout.
    pub fn fit_values(&self, base: f64) {
        for (label, current) in [
            (&self.from_value, &self.from_size),
            (&self.to_value, &self.to_size),
        ] {
            // A few pixels short of the width: at an exact fit the label
            // still cuts the text.
            let width = f64::from(label.width() - 4);
            let width_at = |px: f64| {
                let layout = label.layout().copy();
                layout.set_width(-1);
                layout.set_ellipsize(pango::EllipsizeMode::None);
                let attrs = layout
                    .attributes()
                    .and_then(|a| a.copy())
                    .unwrap_or_else(pango::AttrList::new);
                attrs.change(pango::AttrSize::new_size_absolute(
                    (px * f64::from(pango::SCALE)) as i32,
                ));
                layout.set_attributes(Some(&attrs));
                f64::from(layout.pixel_size().0)
            };
            let mut px = base;
            if width > 1.0 {
                for _ in 0..3 {
                    let text = width_at(px);
                    if text <= width || px <= base * 0.55 {
                        break;
                    }
                    px = (base * 0.55).max(px * width / text);
                }
            }
            let px = (px * 4.0).floor() / 4.0;
            if (px - current.get()).abs() < 0.01 {
                continue;
            }
            current.set(px);
            let attrs = pango::AttrList::new();
            attrs.insert(pango::AttrSize::new_size_absolute(
                (px * f64::from(pango::SCALE)) as i32,
            ));
            label.set_attributes(Some(&attrs));
        }
    }
}
