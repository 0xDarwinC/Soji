export interface Sticker {
    id: number;
    name: string;
    path: string;
    thumbnail_path: string;
    format: string;
    pack: string;
    is_favorite: boolean;
    width: number;
    height: number;
}

export interface AppSettings {
    sticker_path: string;
    recents_limit: number;
    theme: string;
    disable_animations: boolean;
    max_items: number;
    run_on_startup: boolean;
}

export interface IndexingProgress {
    current: number;
    total: number;
    eta_seconds: number | null;
}