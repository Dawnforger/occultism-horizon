# Horizon issue: posted as horizon-rust/horizon#7424 (2026-11-02)

Everything below the line is the issue. It uses only Horizon's own content-mod template
(`cargo horizon-mod new`): nothing from this project, its mod id or its files.

If you post it as written by an agent, Horizon's agent guide asks issues to open with a line like
`> 🤖 Filed by <your GitHub login>'s AI agent (Claude Code, Claude Opus 5.5), not by <your GitHub login> personally.`;
drop it if you rewrite it in your own words. Suggested labels: none (a fix).

---

**Title:** `/loot … mine` says a content mod's block has no loot table

### What happens

`/loot <target> mine <pos> [tool]` on a content mod's block fails, though breaking the same block rolls its
table:

```
/loot spawn ~ ~2 ~ mine <x> <y> <z> minecraft:netherite_pickaxe
Block loot_repro:glowing_amethyst has no loot table
```

On a vanilla block (`minecraft:barrel`) the same command drops `1 [Barrel] from loot table minecraft:blocks/barrel`,
and in survival a netherite pickaxe breaking `loot_repro:glowing_amethyst` drops it.

### Expected

As vanilla: `LootCommand`'s `mine` rolls the broken block's own loot table (`BlockBehaviour.getLootTable`, `<ns>:blocks/<name>`),
whatever its namespace.

### Why

The `mine` arm in `crates/horizon-client/src/game/commands/terrain_cmds.rs` (line 577 on `main` at `0cf1b3d`) builds
the id in the `minecraft` namespace:

```rust
let table = format!("minecraft:blocks/{}", name.trim_start_matches("minecraft:"));
```

so `loot_repro:glowing_amethyst` looks for `minecraft:blocks/loot_repro:glowing_amethyst`. Breaking a block goes
through `Items::block_loot_table(registry, state)` (`horizon-items/src/items.rs`), which keeps the namespace and maps
wall variants to the standing block's table; using it here would make the two agree.

Related: #7408 lets `/loot … loot <id>` name a mod's tables; it doesn't change `mine`.

### Reproduce

1. `cargo horizon-mod new loot-repro && cd loot-repro && cargo horizon-mod build`; put
   `target/horizon-mod/loot_repro.hmod` in the game folder's `horizon-mods/` and turn it on.
2. In a creative world: `/setblock ~2 ~ ~ loot_repro:glowing_amethyst`, then
   `/loot spawn ~ ~2 ~ mine ~2 ~ ~ minecraft:netherite_pickaxe` → "has no loot table".
3. Same with `minecraft:barrel` in its place → drops a barrel.

Seen on Nightly 2026-11-01 (`c9d474d`) and on a build of `0f0fa62`, Linux; the code is unchanged on `main` (`0cf1b3d`).
