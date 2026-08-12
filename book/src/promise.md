# Promise

This page goes deeper than most — `Promise<T>` trips people up more than anything else in the engine, usually because the *mechanism* (why it exists, why it isn't just a resource, why storing it is your job) isn't obvious from the API alone. Read this once and the rest of the engine's async-ish bits (GPU backend acquisition, [`Buffer::read`](./buffers.md#reading-a-buffer-back)) fall out for free.

## The problem

Systems are synchronous functions that run once per tick and are expected to return quickly. There's no `.await` inside a system, and nothing in the scheduler will pause one system while it waits on I/O — if a system blocked, every other system in that stage would freeze with it for that tick.

But some things genuinely take time and can't be forced into "just compute it right now": acquiring a GPU device from the OS, waiting for a GPU-to-CPU buffer copy to land, decoding a file on a background thread. These operations start on one tick and finish on some *later*, unpredictable tick. `Promise<T>` exists to bridge that gap — a way for a system to say "start this, and check back on it every tick without blocking."

## What it actually is

Strip away the ECS integration and `Promise<T>` is nothing more than a labeled oneshot channel:

```rust,ignore
let (fulfiller, promise) = Promise::new();
```

`Fulfiller<T>` is the producer half — hand it off to whatever does the actual work (a background thread, an async task, a GPU driver callback), and it calls `fulfiller.fulfill(value)` exactly once when the result is ready. `Promise<T>` is the consumer half — you keep it, and call `.poll()` on it whenever you want to check in. `poll()` never blocks; it's cheap enough to call every single tick until it resolves:

```rust,ignore
match promise.poll() {
    PromiseState::Pending => {}                // not yet — check again next tick
    PromiseState::Ready(value) => { /* ... */ } // resolved — this only ever happens once
    PromiseState::Disconnected => { /* ... */ } // the Fulfiller was dropped without fulfilling — never resolving
}
```

`Disconnected` isn't a failure path you can usually recover from — it means whatever was supposed to produce the value gave up (panicked, was cancelled) without calling `fulfill`. Treat it like `Ready` in the sense that it's *also* a terminal state: stop polling.

## Why it's not a resource

Every other piece of shared state in pebble — `Time`, `Assets<T>`, `Backend` — is a resource: one instance, globally addressable by type, registered once. A `Promise<T>` doesn't fit that shape. It isn't persistent app state; it's a handle to *one specific pending operation* that exists for a few ticks and then is gone. If `Promise<T>` auto-registered itself as a resource, you'd only ever be able to have one in-flight `Promise<SomeType>` at a time for the entire app, which is far too restrictive — you might want to kick off several unrelated GPU readbacks in the same tick, for instance.

So instead, `Promise<T>` is just a plain value, same as an `i32` or a `String`. Nothing about it is special-cased by the engine. That also means **the engine does not automatically keep it alive between ticks** — which is the part that actually confuses people, so it gets its own section.

## The real challenge: giving it a home

A system function's local variables do not persist between ticks. The scheduler calls your function fresh, every tick:

```rust,ignore
fn broken(/* no persistent parameter */) {
    let (fulfiller, promise) = Promise::new(); // remade from scratch, every single tick
    // by the time this function is called again next tick, `promise` is already gone —
    // you can never see it resolve
}
```

To poll something across multiple ticks, it has to live somewhere that survives between calls of your function. Pebble doesn't have one special place for this — you choose, based on who needs to see the `Promise`:

- **Only one system needs to poll it** → a [`Local<Option<Promise<T>>>`](./systems-and-stages.md#local-state). Private to that one system, persists across its ticks, nobody else can see or interfere with it.
- **Multiple systems need to see the same pending operation** → wrap it in a field on your own resource, and insert/remove that resource as the operation starts/finishes.
- **It belongs to one specific entity** (a per-entity async load, say) → a field on a component.

In every case the shape is the same: wrap it in `Option<Promise<T>>`, start it as `None`, set it to `Some(promise)` when you kick off the operation, and set it back to `None` once `poll()` returns `Ready` or `Disconnected` — otherwise you'd keep matching against an already-resolved (and by then meaningless) channel forever.

## Walkthrough 1: `Local`, one consumer

This is the shape to reach for by default — used internally by [`Buffer::read`](./buffers.md#reading-a-buffer-back), where only the system that kicked off the readback cares about the result:

```rust,ignore
fn readback(buffer: Read<SomeBuffer>, mut pending: Local<Option<Promise<Vec<u8>>>>) {
    // kick it off once
    if pending.is_none() {
        *pending = Some(buffer.0.read());
    }

    // check in every tick after that
    if let Some(p) = pending.as_ref() {
        match p.poll() {
            PromiseState::Ready(bytes) => {
                // do something with `bytes`
                *pending = None; // done — stop polling
            }
            PromiseState::Pending => {}
            PromiseState::Disconnected => {
                *pending = None; // gave up — stop polling
            }
        }
    }
}
```

One system, one `Local`, nobody else involved. This is the pattern for the vast majority of `Promise` usage.

## Walkthrough 2: a resource, multiple systems

Sometimes more than one system needs visibility into the same pending operation — this is exactly the shape pebble itself uses internally to acquire the GPU backend (`src/graphics/render.rs`), and it's worth reading end to end because it shows *why* you'd reach for a resource instead of a `Local`.

Three systems are involved, all running every tick until the backend is ready:

```rust,ignore
// the resource that gives every interested system a shared view of the same Promise
pub struct GPUReceiver {
    promise: Promise<Backend>,
}
```

**System 1 — kicks the operation off, exactly once:**

```rust,ignore
fn obtain_gpu(mut commands: Commands, receiver: Option<Read<GPUReceiver>>, /* ... */) {
    if receiver.is_some() {
        return; // already in flight — don't start a second one
    }
    let (fulfiller, promise) = Promise::new();
    // ...spawn the actual async GPU initialization, which eventually calls
    // fulfiller.fulfill(backend) from wherever it finishes...
    commands.insert_resource(GPUReceiver { promise });
}
```

Because the `Promise` now lives in a resource rather than that system's own `Local`, checking `receiver.is_some()` from *any* system tells you whether one is already in flight — that's the whole reason this needed to be a resource instead of a `Local`: `obtain_gpu` itself needs to see the state that `poll_gpu` (a different system) is updating.

**System 2 — polls it every tick:**

```rust,ignore
fn poll_gpu(mut commands: Commands, receiver: Option<Read<GPUReceiver>>, /* ... */) {
    let Some(receiver) = receiver else { return; };
    match receiver.promise.poll() {
        PromiseState::Ready(backend) => commands.insert_resource(backend), // now Backend exists as its own resource
        PromiseState::Pending => {}
        PromiseState::Disconnected => { /* log and give up */ }
    }
}
```

**System 3 — cleans up the now-useless `GPUReceiver` once the real `Backend` resource exists:**

```rust,ignore
fn clean_up_gpu_acquisition_resources(mut commands: Commands, ready: Read<BackendReady>, receiver: Option<Read<GPUReceiver>>) {
    if ready.0 && receiver.is_some() {
        commands.remove_resource::<GPUReceiver>();
    }
}
```

Notice there's no `Option<Promise<T>>` here at all — the resource itself (`Option<Read<GPUReceiver>>`, i.e. whether the resource exists) plays the role that `Option` played in the `Local` version. Once the backend is ready, the whole `GPUReceiver` resource is removed rather than cleared to `None` internally.

## Walkthrough 3: handing the result to a dependent system via an Event

A common shape: System A dispatches a compute pass, reads the result back, and System B needs that result — but B has no other reason to run except "there's new data from A." Wrapping the result in a resource works, but an [Event](./events.md) is usually the better fit: a readback finishing is a discrete occurrence, not persistent state, and B's whole job is "react when one happens," not "hold onto a value forever."

```rust,ignore
struct ReadbackDone(Vec<u8>);

app.add_event::<ReadbackDone>()

// System A: still owns its own pending Promise privately, in a Local — nothing
// changes about *that* part. The only difference from Walkthrough 1 is what
// happens once it resolves.
fn compute_and_readback(
    backend: Read<Backend>,
    buffer: Read<SomeBuffer>,
    mut pending: Local<Option<Promise<Vec<u8>>>>,
    mut writer: EventWriter<ReadbackDone>,
) {
    if pending.is_none() {
        backend.dispatch_compute(|pass| { /* ... */ });
        *pending = Some(buffer.0.read());
    }

    if let Some(p) = pending.as_ref() {
        match p.poll() {
            PromiseState::Ready(bytes) => {
                writer.send(ReadbackDone(bytes));
                *pending = None;
            }
            PromiseState::Pending => {}
            PromiseState::Disconnected => { *pending = None; }
        }
    }
}

// System B: always scheduled, effectively a no-op on ticks with nothing new
fn use_result(mut reader: EventReader<ReadbackDone>) {
    for event in reader.iter() {
        // ... use event.0 ...
    }
}
```

Two things worth knowing about the timing here, since they're easy to get backwards:

- **`EventWriter::send` isn't deferred.** Unlike `Commands::insert_resource` — which is queued and only takes effect once the whole stage finishes and its commands flush — `send` mutates the underlying `Events<T>` immediately. That means B doesn't need to be pushed to a later *stage* than A to see the same-tick result; being registered after A within the *same* stage is already enough, since a stage's systems run one after another against the same live resources.
- **Events expire; resources don't.** A sent event is visible for the rest of the tick it was sent, plus the following tick, then it's aged out for good — see [Events](./events.md). That's a non-issue for a system like `use_result` above, which is unconditionally scheduled and calls `reader.iter()` every single tick regardless of whether anything's there (pebble has no conditional system scheduling — a registered system always runs, so it can never "miss its turn" and fail to drain the queue). It only becomes a real risk if you ever gate the *consuming logic* on something that could skip calling `.iter()` for a tick or more — in that case, reach for the resource-based shape in Walkthrough 2 instead, since a resource simply waits for you rather than expiring.

## A note on threads

`Promise<T>` carries an `unsafe impl Sync`, which is worth understanding rather than just trusting. The underlying `oneshot::Receiver<T>` is `Send` but not `Sync` — it uses a raw pointer internally, and the crate only guarantees safety for a single consumer touching it, not concurrent access through a shared reference. But `Local<T>`/resource storage both require their contents to be `Send + Sync`, regardless of whether anything is actually reading it concurrently.

This is safe in pebble specifically because the scheduler runs one system at a time, on a single thread, never concurrently — a `Promise` stored in a `Local` or resource is genuinely never touched by two systems at once, even though the type system can't see that guarantee on its own. This `unsafe impl` is pebble asserting a fact about its own scheduler, not a general claim that `Promise<T>` is safe to share across real threads.

## Choosing where to put it: quick reference

| Situation | Storage |
|---|---|
| One system starts it and polls it | `Local<Option<Promise<T>>>` |
| Several systems need to observe the same pending operation, for as long as it takes | A field on your own resource, inserted/removed via `Commands` |
| One or more *other*, unconditionally-scheduled systems just need to react once when it resolves | Keep the `Promise` in a `Local` (Walkthrough 1), and [send an Event](./events.md) once it's `Ready` (Walkthrough 3) |
| The pending operation belongs to one entity | A field on a component |
