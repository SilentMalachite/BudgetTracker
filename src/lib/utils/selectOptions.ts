export type Named = { id: number; name: string; archived_at?: string | null };

export function optionsWithCurrent(
  items: Named[],
  currentId: string,
  includeArchived: boolean,
): { value: string; label: string }[] {
  const visible = items.filter((item) => includeArchived || !item.archived_at);
  const options = visible.map((item) => ({
    value: String(item.id),
    label: item.archived_at ? `${item.name} (アーカイブ済)` : item.name,
  }));
  if (currentId && !options.some((option) => option.value === currentId)) {
    const missing = items.find((item) => String(item.id) === currentId);
    if (missing) {
      options.unshift({
        value: String(missing.id),
        label: `${missing.name} (アーカイブ済)`,
      });
    }
  }
  return options;
}
