use crate::{class_interner::ClassInterner, component::set::ComponentSet};

pub struct Renderer<'a, APP, W> {
    component_set: ComponentSet<'a>,
    class_interner: ClassInterner,
    main_component: APP,
    writer: W,
}

pub struct RendererBuilder<'a, W = ()> {
    component_set: ComponentSet<'a>,
    class_interner: ClassInterner,
    writer: W,
}

impl<'a> Renderer<'a, (), ()> {
    pub fn builder() -> RendererBuilder<'a> {
        RendererBuilder {
            component_set: ComponentSet::new(),
            class_interner: ClassInterner::new(),
            writer: (),
        }
    }
}

impl<'a> RendererBuilder<'a> {
    pub fn with_writer<W>(self, writer: W) -> RendererBuilder<'a, W> {
        RendererBuilder {
            component_set: self.component_set,
            class_interner: self.class_interner,
            writer,
        }
    }
}

impl<'a, W> RendererBuilder<'a, W> {}
