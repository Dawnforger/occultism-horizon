# Storage: design

Occultism's magic storage, rebuilt so that it stays fast in a late-game modpack base. The goal is that
players who like magic never have to switch to a tech storage mod.

## Why Occultism's storage lags

Read from its source (`StorageControllerBlockEntity`, `StorageControllerContainerBase`):

| Cause | Where | Cost |
|---|---|---|
| Every change resends everything | `onContentsChanged` drops the cached `MessageUpdateStacks`; the next update re-encodes every stack and sends the whole list to each viewer | O(all items) per change, per viewer |
| Fuzzy lookups scan every slot | `getItemStack` and `getAvailableAmount` loop over all slots unless the match is exact | O(all items) per query, and automation queries constantly |
| Saving rewrites the whole store | Contents live in the block entity's NBT, re-encoded at every autosave, and copied into the item when picked up | O(all items) per save, in the chunk's own data |
| Each damaged tool is its own type | Durability makes items unique, so a late-game store is thousands of distinct component maps | All of the above multiplied by the worst items |

## Performance rules

Everything below follows from one rule: **the cost of an operation depends on what changed, never on how
much is stored.**

1. **Lookups are hash lookups.** An item's exact identity is its encoded bytes, hashed once. Exact insert and
   extract are O(1). "Any iron sword" looks only at the variants of that item, via a per-item index.
2. **Nothing is sent to a player who isn't looking.** Viewers get one snapshot when they open a terminal, then
   only deltas (what changed since last tick), merged so a burst of changes to one item is one record.
3. **Sorting and searching run on the client,** on the player's own copy of the list. The server never sorts.
4. **Storage isn't in the world's chunk data.** Each store is its own file in the world's folder
   (`occultism/storage/<id>.bin`), so chunk saves stay small and world backups still carry it.
5. **Saves are appended, not rewritten.** The file is a log of "this item now has this count" records. A flush
   appends only what changed; the log is compacted when it has grown well past the live data.
6. **An idle store costs nothing.** No per-tick work scales with contents: no scans, no re-encoding.

`tests/logic` holds scale tests that pin this down with hundreds of thousands of unique items.

## Structure

- **Storage Core + Dimensional Storage Actuator** make the multiblock: the Actuator sits on the Core. The Core
  holds the store; the Actuator is its interface.
- **Plates** (Occultism's stabilizers, tiers 1 to 4) around them raise capacity. Each plate adds Occultism's
  numbers for its tier (64 / 128 / 256 / 512 item types, 512k / 1,024k / 2,048k / 4,096k items) to the base
  128 types and 256k items. They sit in the eight places around the Core and the eight around the Actuator.
- A store has its own id, independent of where the blocks are, so a Core can later carry its contents as a
  small id on its item instead of a giant NBT blob.

Long-term look: the multiblock adapts to the parts added, with plates revolving around the core. Plates are
read as data each scan, so this is drawing work (`block_entity_submit`), not a redesign.

## Resilient pairing

Rebinding every wormhole and accessor after moving a Core is a known pain. The rule here, in the spirit of
Flux Networks: **the network is the lasting thing, and blocks only attach to it.**

- **A store is a world-level object**, identified by its id, with a small record beside its item file: name,
  owner, capacity, where its Core is now. It never depends on a position or a loaded chunk.
- **Everything binds to the id, never to a position.** Wormholes, Accessors, Actuators and Acolytes all hold only
  the store's id, so moving the Core changes one field of one record and nothing else has to know.
- **The Core is an anchor, not the container.** Breaking it gives an item carrying the id (`loot_drops` writes
  it, `place_block` reads it back); the stored items never move. Placing it elsewhere, in any dimension, re-anchors
  the store and every gate stays bound. It is a few bytes on the item, however much is stored.
- **One anchor per store.** A second Core with the same id can't take over a live store (copied items in creative,
  for example).
- **Gates keep working where the Core's chunk isn't loaded,** because the store lives in memory and in its file,
  not in a block entity. Capacity lives in the record, so it survives the plates' chunk being unloaded.
- **It stays magic, not tech.** No network screen and no channels: a store is known by a spirit's true name, and
  gates are attuned by touching them to the Core or Actuator (Occultism already generates spirit names). The
  player sees "bound to Azuriel", not an id.

Multiplayer needs an owner on the record, so that a gate can only be attuned by someone allowed to.

## Access

- **Stable Wormholes stay the logistics answer:** a linked block that behaves like the store for hoppers and
  machines. They talk to a store by its id, so they work at any distance.
- **The Storage Accessor stays the wireless terminal:** Horizon can't open a mod screen from an item yet, so
  its first form may be a paged chest menu; it moves to the full terminal when the API allows.

Both use the same store operations as the Actuator; neither needs a design of its own.

## Roadmap: Acolytes (long term)

Ritual autocrafting that fits the mod's flavour: **one Acolyte per Ritual Book**, a base full of minions.

- A **Ritual Book** records a ritual the player wants repeated: the recipe, and where it is performed.
- An **Acolyte** is given a book. When triggered it fetches the ingredients from the nearest wormhole or
  actuator, performs the ritual, and deposits the result in storage.

What the prototype keeps ready for this:

- **The store's operations are the only way in or out** (`insert`, `extract`, `extract_any`, `count_of`,
  `count_item`), so an Acolyte is simply another client of them, like a hopper or a terminal.
- **Cheap "do we have enough?" queries:** `count_item` is O(variants), so many Acolytes can poll.
- **Reservations** (holding ingredients for a ritual in progress) can sit on top of `extract`.
- **The ritual engine is already data-driven** (`ritual.rs`): a book can name a ritual by id, and `Ritual`
  already lists ingredients, pentacle and result.

The open question is the Acolyte itself: a mod entity with pathing needs native API that Horizon may not have
yet, so the first version could be a stationed block entity that only looks like one.

## Prototype status

Measured in game on Horizon `6475f62`, with 200,000 unique items (about 150 bytes each, a 36 MB file):

| | Time per tick | P95 |
|---|---|---|
| Nearly empty store, idle | 3.0 ms | 3.7 ms |
| 200,000 unique items, idle | 2.7 ms | 3.5 ms |
| 200,000 unique items, 20 inserts a second | 4.4 ms | 5.8 ms |

A world save appended 47 bytes. The first version of the log's compaction check built a snapshot on every
flush (P95 21 ms); `Store::live_bytes` fixed it, and it shows why these numbers are worth re-measuring.

Next: the store's record and portable Cores (see Resilient pairing); the terminal screen with client-side search
and sorting, fed by `drain_viewers`; rituals for the parts. Compaction itself is still O(store), so it should move to a background thread: it runs only when
the log has grown four times past the live data, but a large store would notice that tick.
