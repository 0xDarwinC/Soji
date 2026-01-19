import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Sticker, IndexingProgress, AppSettings } from "../types";

export function useLibrary(settings: AppSettings) {
    const [stickers, setStickers] = useState<Sticker[]>([]);
    const [query, setQuery] = useState("");
    const [activeTab, setActiveTab] = useState("");
    const [packs, setPacks] = useState<string[]>([]);
    const [indexingProgress, setIndexingProgress] = useState<IndexingProgress | null>(null);
    
    // Refs for event listeners to access current state
    const activeTabRef = useRef(activeTab);
    const queryRef = useRef(query);

    useEffect(() => { activeTabRef.current = activeTab; }, [activeTab]);
    useEffect(() => { queryRef.current = query; }, [query]);

    useEffect(() => {
        initLibrary();

        const unlistenShown = listen("app_shown", () => {
            loadStickers(queryRef.current, activeTabRef.current || "All");
        });

        const unlistenUpdate = listen("library_updated", () => {
            setIndexingProgress(null);
            loadStickers(queryRef.current, activeTabRef.current || "All");
        });

        const unlistenProgress = listen<IndexingProgress>("indexing_progress", (event) => {
            setIndexingProgress(event.payload);
        });

        return () => {
            unlistenShown.then(f => f());
            unlistenUpdate.then(f => f());
            unlistenProgress.then(f => f());
        }
    }, []);

    useEffect(() => {
        invoke<string[]>("get_packs").then(setPacks);
    }, [stickers]);

    const initLibrary = async () => {
        try {
            const recents = await invoke<Sticker[]>("search_stickers", { 
                query: "", tab: "Recents", limit: 1 
            });
            if (recents.length > 0) {
                setActiveTab("Recents");
                loadStickers("", "Recents");
            } else {
                setActiveTab("All");
                loadStickers("", "All");
            }
        } catch {
            setActiveTab("All");
            loadStickers("", "All");
        }
    };

    const loadStickers = async (searchQuery: string, currentTab: string) => {
        if (!currentTab) return;
        // if -1, infinite
        const limit = settings.max_items === -1 ? 999999 : settings.max_items;
        try {
            const result = await invoke<Sticker[]>("search_stickers", {
                query: searchQuery,
                tab: currentTab,
                limit: limit
            });
            setStickers(result);
        } catch (err) {
            console.error(err);
        }
    };

    const handleSearch = (val: string) => {
        setQuery(val);
        loadStickers(val, activeTab);
    };

    const handleTabClick = (pack: string) => {
        setActiveTab(pack);
        loadStickers(query, pack);
    };

    const refreshLibrary = async () => {
        await invoke("refresh_library");
    };

    const reloadCurrentView = () => {
        loadStickers(query, activeTab);
    };

    return {
        stickers,
        query,
        activeTab,
        packs,
        indexingProgress,
        handleSearch,
        handleTabClick,
        refreshLibrary,
        reloadCurrentView
    };
}