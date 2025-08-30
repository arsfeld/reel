use objc2::runtime::AnyObject;
use objc2::{Encode, Encoding, msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{NSLayoutAttribute, NSLayoutConstraint, NSLayoutRelation, NSView};
use objc2_foundation::{NSArray, NSObject};

/// Helper utilities for working with Auto Layout constraints
pub struct AutoLayout;

impl AutoLayout {
    /// Create constraints to pin a view to all edges of its superview
    pub fn pin_to_edges(view: &NSView, insets: NSEdgeInsets) -> Vec<Retained<NSLayoutConstraint>> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview =
            unsafe { view.superview() }.expect("View must have a superview to pin edges");

        vec![
            Self::constraint(
                view,
                NSLayoutAttribute::Top,
                NSLayoutRelation::Equal,
                Some(&superview),
                NSLayoutAttribute::Top,
                1.0,
                insets.top,
            ),
            Self::constraint(
                view,
                NSLayoutAttribute::Leading,
                NSLayoutRelation::Equal,
                Some(&superview),
                NSLayoutAttribute::Leading,
                1.0,
                insets.left,
            ),
            Self::constraint(
                view,
                NSLayoutAttribute::Trailing,
                NSLayoutRelation::Equal,
                Some(&superview),
                NSLayoutAttribute::Trailing,
                1.0,
                -insets.right,
            ),
            Self::constraint(
                view,
                NSLayoutAttribute::Bottom,
                NSLayoutRelation::Equal,
                Some(&superview),
                NSLayoutAttribute::Bottom,
                1.0,
                -insets.bottom,
            ),
        ]
    }

    /// Create a single constraint
    pub fn constraint(
        item: &NSView,
        attribute1: NSLayoutAttribute,
        relation: NSLayoutRelation,
        to_item: Option<&NSView>,
        attribute2: NSLayoutAttribute,
        multiplier: f64,
        constant: f64,
    ) -> Retained<NSLayoutConstraint> {
        unsafe {
            // Cast NSView to AnyObject for the constraint API
            let to_item_obj: Option<&AnyObject> = to_item.map(|v| v as &AnyObject);

            NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                item,
                attribute1,
                relation,
                to_item_obj,
                attribute2,
                multiplier,
                constant,
            )
        }
    }

    /// Center a view horizontally in its superview
    pub fn center_horizontally(view: &NSView) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview = unsafe { view.superview() }.expect("View must have a superview to center");

        Self::constraint(
            view,
            NSLayoutAttribute::CenterX,
            NSLayoutRelation::Equal,
            Some(&superview),
            NSLayoutAttribute::CenterX,
            1.0,
            0.0,
        )
    }

    /// Center a view vertically in its superview
    pub fn center_vertically(view: &NSView) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview = unsafe { view.superview() }.expect("View must have a superview to center");

        Self::constraint(
            view,
            NSLayoutAttribute::CenterY,
            NSLayoutRelation::Equal,
            Some(&superview),
            NSLayoutAttribute::CenterY,
            1.0,
            0.0,
        )
    }

    /// Set fixed width constraint
    pub fn width(view: &NSView, width: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        Self::constraint(
            view,
            NSLayoutAttribute::Width,
            NSLayoutRelation::Equal,
            None,
            NSLayoutAttribute::NotAnAttribute,
            1.0,
            width,
        )
    }

    /// Set fixed height constraint
    pub fn height(view: &NSView, height: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        Self::constraint(
            view,
            NSLayoutAttribute::Height,
            NSLayoutRelation::Equal,
            None,
            NSLayoutAttribute::NotAnAttribute,
            1.0,
            height,
        )
    }

    /// Activate multiple constraints at once
    pub fn activate(constraints: &[Retained<NSLayoutConstraint>]) {
        // Convert Retained<T> to &T for from_slice
        let refs: Vec<&NSLayoutConstraint> = constraints.iter().map(|c| &**c).collect();
        let array = NSArray::from_slice(&refs);
        unsafe {
            NSLayoutConstraint::activateConstraints(&array);
        }
    }

    /// Deactivate multiple constraints at once
    pub fn deactivate(constraints: &[Retained<NSLayoutConstraint>]) {
        // Convert Retained<T> to &T for from_slice
        let refs: Vec<&NSLayoutConstraint> = constraints.iter().map(|c| &**c).collect();
        let array = NSArray::from_slice(&refs);
        unsafe {
            NSLayoutConstraint::deactivateConstraints(&array);
        }
    }

    /// Create constraint for top edge
    pub fn top(view: &NSView, constant: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview = unsafe { view.superview() }.expect("View must have a superview");

        Self::constraint(
            view,
            NSLayoutAttribute::Top,
            NSLayoutRelation::Equal,
            Some(&superview),
            NSLayoutAttribute::Top,
            1.0,
            constant,
        )
    }

    /// Create constraint for leading edge
    pub fn leading(view: &NSView, constant: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview = unsafe { view.superview() }.expect("View must have a superview");

        Self::constraint(
            view,
            NSLayoutAttribute::Leading,
            NSLayoutRelation::Equal,
            Some(&superview),
            NSLayoutAttribute::Leading,
            1.0,
            constant,
        )
    }

    /// Create constraint for trailing edge
    pub fn trailing(view: &NSView, constant: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview = unsafe { view.superview() }.expect("View must have a superview");

        Self::constraint(
            view,
            NSLayoutAttribute::Trailing,
            NSLayoutRelation::Equal,
            Some(&superview),
            NSLayoutAttribute::Trailing,
            1.0,
            constant,
        )
    }

    /// Create constraint for bottom edge
    pub fn bottom(view: &NSView, constant: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let superview = unsafe { view.superview() }.expect("View must have a superview");

        Self::constraint(
            view,
            NSLayoutAttribute::Bottom,
            NSLayoutRelation::Equal,
            Some(&superview),
            NSLayoutAttribute::Bottom,
            1.0,
            constant,
        )
    }

    /// Create constraints for aspect ratio
    pub fn aspect_ratio(view: &NSView, ratio: f64) -> Retained<NSLayoutConstraint> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        Self::constraint(
            view,
            NSLayoutAttribute::Width,
            NSLayoutRelation::Equal,
            Some(view),
            NSLayoutAttribute::Height,
            ratio,
            0.0,
        )
    }

    /// Create constraints to match size with another view
    pub fn match_size(view: &NSView, with: &NSView) -> Vec<Retained<NSLayoutConstraint>> {
        unsafe {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        vec![
            Self::constraint(
                view,
                NSLayoutAttribute::Width,
                NSLayoutRelation::Equal,
                Some(with),
                NSLayoutAttribute::Width,
                1.0,
                0.0,
            ),
            Self::constraint(
                view,
                NSLayoutAttribute::Height,
                NSLayoutRelation::Equal,
                Some(with),
                NSLayoutAttribute::Height,
                1.0,
                0.0,
            ),
        ]
    }
}

/// Edge insets for padding
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct NSEdgeInsets {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

unsafe impl Encode for NSEdgeInsets {
    const ENCODING: Encoding = Encoding::Struct(
        "NSEdgeInsets",
        &[
            Encoding::Double,
            Encoding::Double,
            Encoding::Double,
            Encoding::Double,
        ],
    );
}

impl NSEdgeInsets {
    pub fn new(top: f64, left: f64, bottom: f64, right: f64) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    pub fn uniform(inset: f64) -> Self {
        Self {
            top: inset,
            left: inset,
            bottom: inset,
            right: inset,
        }
    }

    pub fn zero() -> Self {
        Self {
            top: 0.0,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        }
    }
}
