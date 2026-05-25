import { invoke } from '@tauri-apps/api/core';

export type CategoryType = 'income' | 'expense';

export type Category = {
  id: number;
  name: string;
  type: CategoryType;
  color: string | null;
  icon: string | null;
  display_order: number;
  archived_at: string | null;
};

export type ListCategoryFilter = {
  type?: CategoryType;
  include_archived?: boolean;
};

export function listCategories(filter: ListCategoryFilter = {}): Promise<Category[]> {
  return invoke<Category[]>('list_categories', { filter });
}

export type CreateCategoryInput = {
  name: string;
  type: CategoryType;
  color?: string;
  icon?: string;
};

export function createCategory(input: CreateCategoryInput): Promise<Category> {
  return invoke<Category>('create_category', { input });
}

export type UpdateCategoryPatch = {
  name?: string;
  color?: string | null;
  icon?: string | null;
  display_order?: number;
};

export function updateCategory(
  id: number,
  patch: UpdateCategoryPatch,
): Promise<Category> {
  return invoke<Category>('update_category', { id, patch });
}

export function archiveCategory(id: number): Promise<void> {
  return invoke('archive_category', { id });
}

export function unarchiveCategory(id: number): Promise<void> {
  return invoke('unarchive_category', { id });
}
