import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { listen } from "@tauri-apps/api/event";
import { useSettings } from "./hooks/useSettings";
import { useLibrary } from "./hooks/useLibrary";
import { SearchBar } from "./components/SearchBar";
import { Header } from "./components/Header/Header";
import { LoadingOverlay } from "./components/LoadingOverlay";
import { SettingsModal } from "./components/Settings/SettingsModal";
import { StickerGrid } from "./components/StickerGrid/StickerGrid";
import { ask } from '@tauri-apps/plugin-dialog';
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { StickerEditorModal, EditorData } from "./components/StickerEditor/StickerEditorModal";
import { Sticker } from "./types";

function App() {
    const { settings, showSettings, setShowSettings, saveSettings, toggleSettings } = useSettings();
    const {
        stickers, query, activeTab, packs, indexingProgress,
        handleSearch, handleTabClick, reloadCurrentView, refreshLibrary
    } = useLibrary(settings);

    const tabsRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);

    const [editorData, setEditorData] = useState<EditorData | null>(null);
    const [isDragging, setIsDragging] = useState(false);

    // Initial focus
    useEffect(() => {
        setTimeout(() => {
            inputRef.current?.focus();
            inputRef.current?.select();
        }, 50);
    }, []);

    useEffect(() => {
        const checkForUpdates = async () => {
            try {
                const update = await check();

                if (update) {
                    console.log(`Found update: ${update.version} from ${update.date}`);

                    const yes = await ask(
                        `Update to ${update.version} is available!\n\nRelease notes: ${update.body}`,
                        {
                            title: 'Update Available',
                            kind: 'info',
                            okLabel: 'Update',
                            cancelLabel: 'Later'
                        }
                    );
                    if (yes) {
                        await update.downloadAndInstall();
                        await relaunch();
                    }
                }
            } catch (err) {
                console.error("Failed to check for updates:", err);
            }
        };

        // checks every hour
        checkForUpdates();
        const interval = setInterval(checkForUpdates, 1000 * 60 * 60);

        return () => clearInterval(interval);
    }, []);

    // drag and drop
    useEffect(() => {
        let dragCounter = 0;


        const handleDragEnter = (e: DragEvent) => {
            e.preventDefault();
            dragCounter++;
            if (dragCounter === 1) setIsDragging(true);
        };

        const handleDragLeave = (e: DragEvent) => {
            e.preventDefault();
            dragCounter--;
            if (dragCounter === 0) setIsDragging(false);
        };

        const handleDragOver = (e: DragEvent) => e.preventDefault();
        
        const handleDrop = (e: DragEvent) => {
            e.preventDefault();
            dragCounter = 0;
            setIsDragging(false);

            const uriData = e.dataTransfer?.getData("text/uri-list");
            const textData = e.dataTransfer?.getData("text/plain");
            const payload = uriData || textData;

            if (payload && (payload.startsWith("http") || payload.startsWith("file"))) {
                processDroppedPayload(payload);
            }
        };

        window.addEventListener('dragenter', handleDragEnter);
        window.addEventListener('dragleave', handleDragLeave);
        window.addEventListener('dragover', handleDragOver);
        window.addEventListener('drop', handleDrop);


        const unlistenDrop = listen('tauri://drag-drop', (event: any) => {
            setIsDragging(false);
            dragCounter = 0;
            
            if (event.payload.paths && event.payload.paths.length > 0) {
                const firstFile = event.payload.paths[0];
                processDroppedPayload(firstFile);
            }
        });

        const unlistenEnter = listen('tauri://drag-enter', () => {
            setIsDragging(true);
        });

        const unlistenLeave = listen('tauri://drag-leave', () => {
            setIsDragging(false);
        });

        return () => {
            window.removeEventListener('dragenter', handleDragEnter);
            window.removeEventListener('dragleave', handleDragLeave);
            window.removeEventListener('dragover', handleDragOver);
            window.removeEventListener('drop', handleDrop);
            unlistenDrop.then(f => f());
            unlistenEnter.then(f => f());
            unlistenLeave.then(f => f());
        };
    }, []);

    const processDroppedPayload = async (payload: string) => {
        try {
            console.log("Processing Drop:", payload);
            const result: any = await invoke('cache_dropped_item', { payload });

            setEditorData({
                mode: 'create',
                filePath: result.temp_path,
                currentName: "New Sticker",
                currentPack: activeTab !== "Recents" && activeTab !== "Favorites" && activeTab !== "All" ? activeTab : "",
                isFavorite: false
            });
        } catch (e) {
            console.error(e);
            alert("Failed to process image: " + e);
        }
    };

    const handleEditRequest = (sticker: Sticker) => {
        setEditorData({
            mode: 'edit',
            filePath: sticker.path,
            currentName: sticker.name,
            currentPack: sticker.pack
        });
    };

    const scrollTags = (direction: 'left' | 'right') => {
        if (tabsRef.current) {
            const scrollAmount = 200;
            tabsRef.current.scrollBy({
                left: direction === 'left' ? -scrollAmount : scrollAmount,
                behavior: 'smooth'
            });
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === "Enter" && stickers.length > 0) {
            invoke("select_sticker", { path: stickers[0].path }).catch(console.error);
        }
        if (e.key === "Escape") {
            invoke("hide_window").catch(() => { });
        }
    };

    const handleClose = () => invoke("hide_window");

    return (
        <div
            className={settings.disable_animations ? "no-animations" : ""}
            onContextMenu={(e) => e.preventDefault()}
            style={{
                height: "100%", width: "100%", overflow: "hidden",
                boxSizing: "border-box", background: "transparent",
                display: "flex", flexDirection: "column", position: "relative",
                color: "white"
            }}
        >
            <Header onToggleSettings={toggleSettings} onRefresh={reloadCurrentView} onClose={handleClose} />

            {indexingProgress && <LoadingOverlay progress={indexingProgress} />}

            {/* DRAG OVERLAY */}
            {isDragging && (
                <div className="drag-overlay">
                    <div className="drag-box">
                        Drop Image Here
                    </div>
                </div>
            )}

            {/* EDITOR MODAL */}
            {editorData && (
                <StickerEditorModal 
                    data={editorData}
                    packs={packs}
                    onClose={() => setEditorData(null)}
                    onSuccess={() => {
                        reloadCurrentView();
                        refreshLibrary();
                    }}
                />
            )}

            {showSettings && (
                <SettingsModal
                    settings={settings}
                    onSaveSettings={saveSettings}
                    onClose={() => setShowSettings(false)}
                    onRefreshRequest={refreshLibrary}
                />
            )}

            {!showSettings && (
                <div style={{ flex: 1, display: "flex", flexDirection: "column", padding: "0 20px 20px 20px", overflow: "hidden" }}>
                    {/* SEARCH */}
                    <SearchBar
                        ref={inputRef}
                        query={query}
                        onSearch={handleSearch}
                        onKeyDown={handleKeyDown}
                    />
                    {/* TAGS NAVIGATION */}
                    {query.length === 0 && (
                        <div style={{ display: "flex", alignItems: "center", gap: "5px", marginBottom: "10px" }}>
                            <button className="tag-nav-btn" onClick={() => scrollTags('left')}>‹</button>

                            <div
                                ref={tabsRef}
                                style={{
                                    flex: 1, display: "flex", gap: "8px", overflowX: "auto",
                                    whiteSpace: "nowrap", paddingBottom: "5px", scrollBehavior: "smooth",
                                    maskImage: "linear-gradient(to right, transparent, black 10px, black 95%, transparent)"
                                }}
                            >
                                {["Recents", "Favorites", "All", ...packs].map(pack => (
                                    <button key={pack} onClick={() => handleTabClick(pack)}
                                        style={{
                                            padding: "6px 14px", borderRadius: "20px", border: "none",
                                            fontSize: "13px", cursor: "pointer",
                                            background: activeTab === pack ? "white" : "rgba(255,255,255,0.1)",
                                            color: activeTab === pack ? "black" : "white",
                                            transition: "all 0.2s", fontWeight: activeTab === pack ? "bold" : "normal",
                                            flexShrink: 0
                                        }}
                                    >
                                        {pack}
                                    </button>
                                ))}
                            </div>

                            <button className="tag-nav-btn" onClick={() => scrollTags('right')}>›</button>
                        </div>
                    )}

                    <StickerGrid stickers={stickers} packs={packs} onReload={reloadCurrentView} onEdit={handleEditRequest} />
                </div>
            )}
        </div>
    );
}

export default App;