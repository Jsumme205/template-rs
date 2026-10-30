//
//
// API design
//
//  Css::select("button".by_id()).text_color(Color::RED)
//
//
//

pub trait Selector {
    #[doc(hidden)]
    const PREFIX: Option<&'static str> = None;
}

pub enum ByElement {}
