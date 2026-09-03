<script lang="ts">
  import EmptyState from './EmptyState.svelte';

  /**
   * カードがまだ実データを描けない間のプレースホルダー。「読み込み中」と
   * 「失敗した」は見た目上どちらも同じ「まだ結果が無い」状態で、違いは
   * メッセージだけなので、真偽の理由（loading か否か）は問わず error の
   * 有無だけで分岐する。9枚のカードすべてがこの1コンポーネントを指すことで、
   * 一部のカードだけ tri-state を書き忘れる、という抜けを構造的に防ぐ。
   */
  let { error }: { error: string | null } = $props();
</script>

{#if error}
  <EmptyState title="読み込みに失敗しました" hint={error} />
{:else}
  <EmptyState title="読み込み中" hint="集計を取得しています" />
{/if}
