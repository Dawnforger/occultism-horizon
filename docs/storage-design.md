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

- **One block, Occultism's Dimensional Storage Actuator,** holds a store and is its terminal: its own model
  (Occultism's pedestal), with Occultism's dimensional matrix floating a block and a quarter above it, turning and
  cycling colour as Occultism's `StorageControllerGeoRenderer` draws it (27 small cubes, drawn by the native mod).
- **Stabilizer plates** (tiers 1 to 4, Occultism's numbers: 64 / 128 / 256 / 512 types, 512k / 1,024k / 2,048k /
  4,096k items each, over a base of 128 types and 256k items) are **upgrades**, not blocks: shift-click one into
  the column of six upgrade slots on the terminal's right, click one to take it back. They belong to the store's
  record, so they move with the Actuator, never drop, and come back with a recall.
- **The projection shows the upgrades:** the matrix is a nucleus, and each plate a chalk glyph circling it like an
  electron, on an orbit of its own (six tilted orbits that cross like an atom's), wider and faster for higher tiers,
  in its tier's chalk colour: white, yellow, purple, red (`network::glyph_orbit`, `glyph_colour`). The glyphs
  are billboards: turned to the camera every frame (from `Host::camera`), as particles are, so they read flat from
  any side. Drawn at full light; Horizon has no custom-texture particles, and these need none.
- A store has its own id, independent of where its block is, so the Actuator carries its contents as a small id
  on its item instead of a giant NBT blob.

## Resilient pairing

Rebinding every wormhole and accessor after moving a Core is a known pain. The rule here, in the spirit of
Flux Networks: **the network is the lasting thing, and blocks only attach to it.**

- **A store is a world-level object**, identified by its id, with a small record beside its item file: name,
  owner, capacity, where its Core is now. It never depends on a position or a loaded chunk.
- **Everything binds to the id, never to a position.** Wormholes, Accessors, Actuators and Acolytes all hold only
  the store's id, so moving the Core changes one field of one record and nothing else has to know.
- **The Actuator ("Core" here) is an anchor, not the container.** Breaking it gives an item carrying the id (`loot_drops` writes
  it, `place_block` reads it back); the stored items never move. Placing it elsewhere, in any dimension, re-anchors
  the store and every gate stays bound. It is a few bytes on the item, however much is stored.
- **Without an anchor a store is dormant.** While its Core is an item, every gate attuned to it stops and says
  the spirit is unbound; nothing moves in or out. Placing the Core wakes them all, with no rebinding, and no
  items are at risk either way.
- **One anchor per store.** A second Core with the same id can't take over a live store (copied items in creative,
  for example).
- **An anchored store works where the Core's chunk isn't loaded,** because the store lives in memory and in its file,
  not in a block entity. Capacity lives in the record, so it survives the plates' chunk being unloaded.
- **It stays magic, not tech.** No network screen and no channels: a store is known by a spirit's true name, and
  gates are attuned by touching them to the Core or Actuator (Occultism already generates spirit names). The
  player sees "bound to Azuriel", not an id.

Multiplayer needs an owner on the record, so that a gate can only be attuned by someone allowed to.

## Ownership and abuse

A store is valuable and its Core is its key, so the rules assume someone will try to steal, copy or
crash their way to more items. Everything below is checked on the server; the client only ever asks.

| Attempt | What stops it |
|---|---|
| Opening someone's terminal | It doesn't open: "<name> does not answer you." The server also refuses to stream the store or hand items to anyone it doesn't trust, whatever the client sends |
| Breaking someone's Core (to make their store dormant, or carry it off) | Refused for anyone the store doesn't trust; operators (level 2+) may |
| Placing a stolen Core | It stays a dead stone and the store stays dormant, so its owner can still recall it |
| Recalling someone else's store | The recall only answers its caster's own stores |
| Keeping an old Core after a recall | The recall raises the store's bond; an older Core binds a new, empty spirit |
| Copying a Core (creative) | A copy of a placed Core binds a new, empty spirit: one key reaches one store |
| Forged grid clicks | A click names a serial; the server checks it is a key of that very store and takes no more than is stored |
| Taking the last of a type into a full inventory | What doesn't fit goes back under the same key, past the store's limits, never lost |
| Crashing or killing the game right after putting items in | Stores and records are written only when the world saves, so after a crash both come back as they were at that save: the items are back in the inventory and not in the store |

**Trust.** A store's owner is whoever first placed its Core (stores from before owners are claimed the same way).
`/occultism trust <player>` and `untrust <player>` act on the store the owner last opened; `/occultism trusted`
lists who it trusts. Trusted players open its terminal, break and move its Core and wake it; only the owner
recalls it or changes who it trusts. Hoppers may still put items into any store: a gift, never a way out.

Not covered: a claim or protection mod's job (someone breaking the Actuator or blocks around a store), and an
operator's own tools (creative inventory, commands, editing files).

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

Portable Cores are in (`network.rs`, `vault.rs`): each store has a record, `occultism/storage/<id>.txt`, of a few
lines (spirit name, owner, capacity, anchor), rewritten only when a Core moves or its plates change. A broken Core
drops as an item with an `occultism:store` component (`{id, name}`) and the store goes dormant; placing it wakes
the store wherever it lands. The Core always drops (even in an explosion, which it resists anyway) and its item
doesn't burn; a Core item lost some other way (despawned, voided) leaves the store dormant on disk, so a recall
ritual is the natural later fix.

The terminal is in (`terminal.rs` for its rules, `terminal_ui.rs` for the screen): opening an Actuator sends the
viewer one snapshot, over a mod channel in messages of at most 29 kB, 16 a tick, so even a huge store streams in
over a few seconds without a spike; after that a server tick sends only the keys that changed, to everyone watching
that store. Each key gets a small serial, so a click on the grid names it in a menu button. Search (words in the
name or id, `@mod` for a mod), sorting (count, name, mod) and scrolling are the client's alone, re-sorted at most
five times a second while a store changes. Items leave the terminal straight to the player's inventory (left: a
stack, right: one); items go in through the input row, hoppers or shift-clicks.

**Recall.** Each store's record has a **bond**, carried on its Core item. The recall ritual (Craft Foliot, the
Foliot book, an ender pearl, a spirit-attuned gem, silver and amethyst, 30 s) finds every store of the caster with
no Core placed (dormant, or its anchor gone) and none of whose current Cores the caster carries; each gets a new
Core, its bond raised, dropped on the golden bowl. A Core item from before (in a chest, say) no longer reaches the
store when placed and binds a new spirit instead, so a recall never makes two live keys to one store. Stores from
before owners are claimed by whoever next places their Core.

Next: wormholes and the Storage Accessor; rituals for the parts. Compaction itself is still O(store), so it should move to a background thread: it runs only when
the log has grown four times past the live data, but a large store would notice that tick.
