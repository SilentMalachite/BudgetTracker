<script lang="ts">
  import { onDestroy } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import CategoryBadge from '../lib/components/CategoryBadge.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import Select from '../lib/components/Select.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import {
    archiveCategory,
    createCategory,
    unarchiveCategory,
    updateCategory,
    type Category,
    type CategoryType,
  } from '../lib/api/categories';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';

  const store = createCategoriesStore({ include_archived: true });
  onDestroy(() => {
    void store.dispose();
  });

  let modalOpen = $state(false);
  let editing = $state<Category | null>(null);
  let name = $state('');
  let type = $state<CategoryType>('expense');
  let color = $state('');
  let icon = $state('');
  let formError = $state<string | null>(null);

  function openCreate() {
    editing = null;
    name = '';
    type = 'expense';
    color = '';
    icon = '';
    formError = null;
    modalOpen = true;
  }

  function openEdit(category: Category) {
    editing = category;
    name = category.name;
    type = category.type;
    color = category.color ?? '';
    icon = category.icon ?? '';
    formError = null;
    modalOpen = true;
  }

  async function submit() {
    formError = null;
    try {
      if (editing) {
        await updateCategory(editing.id, {
          name,
          color: color.length > 0 ? color : null,
          icon: icon.length > 0 ? icon : null,
        });
      } else {
        await createCategory({
          name,
          type,
          color: color.length > 0 ? color : undefined,
          icon: icon.length > 0 ? icon : undefined,
        });
      }
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleArchive(category: Category) {
    if (category.archived_at) await unarchiveCategory(category.id);
    else await archiveCategory(category.id);
  }

  const visible = $derived(store.items.filter((category) => !category.archived_at));
  const archived = $derived(store.items.filter((category) => !!category.archived_at));
</script>

<section>
  <header class="page-header">
    <h1>カテゴリ</h1>
    <Button onclick={openCreate}>
      {#snippet children()}+ 追加{/snippet}
    </Button>
  </header>

  <Card>
    {#snippet children()}
      {#if store.loading && store.items.length === 0}
        <p>読み込み中...</p>
      {:else if visible.length === 0}
        <EmptyState title="カテゴリがありません" hint="右上の「+ 追加」から作成してください" />
      {:else}
        <ul class="list" data-testid="categories-list">
          {#each visible as category (category.id)}
            <li>
              <CategoryBadge {category} />
              <small class="type">{category.type === 'income' ? '収入' : '支出'}</small>
              <span class="spacer"></span>
              <Button variant="ghost" onclick={() => openEdit(category)}>
                {#snippet children()}編集{/snippet}
              </Button>
              <Button variant="ghost" onclick={() => toggleArchive(category)}>
                {#snippet children()}アーカイブ{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  {#if archived.length > 0}
    <h2 class="sub">アーカイブ済み</h2>
    <Card>
      {#snippet children()}
        <ul class="list">
          {#each archived as category (category.id)}
            <li>
              <CategoryBadge {category} />
              <small class="type">{category.type === 'income' ? '収入' : '支出'}</small>
              <span class="spacer"></span>
              <Button variant="ghost" onclick={() => toggleArchive(category)}>
                {#snippet children()}復元{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </Card>
  {/if}
</section>

<Modal
  open={modalOpen}
  title={editing ? 'カテゴリを編集' : 'カテゴリを追加'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <TextField label="名前" required bind:value={name} testid="category-name" />
    {#if !editing}
      <Select
        label="種別"
        required
        bind:value={type}
        options={[
          { value: 'expense', label: '支出' },
          { value: 'income', label: '収入' },
        ]}
        testid="category-type"
      />
    {/if}
    <TextField label="色 (#RRGGBB)" bind:value={color} placeholder="#FF00AA" />
    <TextField label="アイコン (絵文字)" bind:value={icon} placeholder="🍱" />
    {#if formError}<small class="error">{formError}</small>{/if}
  {/snippet}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (modalOpen = false)}>
      {#snippet children()}キャンセル{/snippet}
    </Button>
    <Button onclick={() => void submit()}>
      {#snippet children()}{editing ? '更新' : '追加'}{/snippet}
    </Button>
  {/snippet}
</Modal>

<style>
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--space-5);
  }

  h1 {
    color: white;
    margin: 0;
  }

  h2.sub {
    color: white;
    margin: var(--space-6) 0 var(--space-4);
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: var(--space-3);
  }

  .list li {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: rgba(0, 0, 0, 0.02);
  }

  .type {
    color: var(--muted);
  }

  .spacer {
    flex: 1;
  }

  .error {
    color: var(--danger);
  }
</style>
