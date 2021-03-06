use crate::actor::start_actor;
use crate::{Actor, Addr, Context};
use fnv::FnvHasher;
use futures::lock::Mutex;
use once_cell::sync::OnceCell;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// Trait define a global service.
///
/// The service is a global actor.
/// You can use `Actor::from_registry` to get the address `Addr<A>` of the service.
///
/// # Examples
///
/// ```rust
/// use xactor::*;
///
/// #[message(result = "i32")]
/// struct AddMsg(i32);
///
/// #[derive(Default)]
/// struct MyService(i32);
///
/// impl Actor for MyService {}
///
/// impl Service for MyService {}
///
/// #[async_trait::async_trait]
/// impl Handler<AddMsg> for MyService {
///     async fn handle(&mut self, ctx: &Context<Self>, msg: AddMsg) -> i32 {
///         self.0 += msg.0;
///         self.0
///     }
/// }
///
/// #[async_std::main]
/// async fn main() -> Result<()> {
///     let mut addr = MyService::from_registry().await;
///     assert_eq!(addr.call(AddMsg(1)).await?, 1);
///     assert_eq!(addr.call(AddMsg(5)).await?, 6);
///     Ok(())
/// }
/// ```
#[async_trait::async_trait]
pub trait Service: Actor + Default {
    async fn from_registry() -> Addr<Self> {
        static REGISTRY: OnceCell<
            Mutex<HashMap<TypeId, Box<dyn Any + Send>, BuildHasherDefault<FnvHasher>>>,
        > = OnceCell::new();
        let registry = REGISTRY.get_or_init(|| Default::default());
        let mut registry = registry.lock().await;

        match registry.get_mut(&TypeId::of::<Self>()) {
            Some(addr) => addr.downcast_ref::<Addr<Self>>().unwrap().clone(),
            None => {
                let (ctx, rx) = Context::new();
                registry.insert(TypeId::of::<Self>(), Box::new(ctx.address()));
                drop(registry);
                let addr = ctx.address();
                start_actor(ctx, rx, Self::default(), true).await;
                addr
            }
        }
    }
}

thread_local! {
    static LOCAL_REGISTRY: RefCell<HashMap<TypeId, Box<dyn Any + Send>, BuildHasherDefault<FnvHasher>>> = Default::default();
}

/// Trait define a local service.
///
/// The service is a thread local actor.
/// You can use `Actor::from_registry` to get the address `Addr<A>` of the service.
#[async_trait::async_trait]
pub trait LocalService: Actor + Default {
    async fn from_registry() -> Addr<Self> {
        let res = LOCAL_REGISTRY.with(|registry| {
            registry
                .borrow_mut()
                .get_mut(&TypeId::of::<Self>())
                .map(|addr| addr.downcast_ref::<Addr<Self>>().unwrap().clone())
        });
        match res {
            Some(addr) => addr,
            None => {
                let addr = {
                    let (ctx, rx) = Context::new();
                    let addr = ctx.address();
                    start_actor(ctx, rx, Self::default(), true).await;
                    addr
                };
                LOCAL_REGISTRY.with(|registry| {
                    registry
                        .borrow_mut()
                        .insert(TypeId::of::<Self>(), Box::new(addr.clone()));
                });
                addr
            }
        }
    }
}
