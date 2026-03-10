import { useEffect, useRef, useState, useCallback } from "react";
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

let isUpdateDialogOpen = false;

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

    const appStateRef = useRef({ query, activeTab, handleSearch, handleTabClick, reloadCurrentView });
    useEffect(() => {
        appStateRef.current = { query, activeTab, handleSearch, handleTabClick, reloadCurrentView };
    }, [query, activeTab, handleSearch, handleTabClick, reloadCurrentView]);

    useEffect(() => {
        appStateRef.current = { query, activeTab, handleSearch, handleTabClick, reloadCurrentView };
    }, [query, activeTab, handleSearch, handleTabClick, reloadCurrentView]);

    useEffect(() => {
        setTimeout(() => {
            inputRef.current?.focus();
            inputRef.current?.select();
        }, 50);

        const unlistenAppShown = listen('app_shown', () => {
            const current = appStateRef.current;
            const needsTabReset = current.activeTab !== "Recents";

            if (current.query !== "") {
                current.handleSearch("");
            }

            if (needsTabReset) {
                setTimeout(() => {
                    appStateRef.current.handleTabClick("Recents");
                }, 10);
            } else {
                appStateRef.current.reloadCurrentView();
            }

            setTimeout(() => {
                inputRef.current?.focus();
                inputRef.current?.select();
            }, 50);
        });

        return () => {
            unlistenAppShown.then(f => f());
        };
    }, []);

    useEffect(() => {
        const checkForUpdates = async () => {
            if (isUpdateDialogOpen) return;
            try {
                const update = await check();

                if (update) {
                    isUpdateDialogOpen = true;
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
                    isUpdateDialogOpen = false;
                }
            } catch (err) {
                isUpdateDialogOpen = false;
                console.error("Failed to check for updates:", err);
            }
        };

        // checks every hour
        checkForUpdates();
        const interval = setInterval(checkForUpdates, 1000 * 60 * 60);

        return () => clearInterval(interval);
    }, []);

    // drag and drop
    const processDroppedPayload = useCallback(async (payload: string, htmlData?: string) => {
        try {
            let cleanPayload = payload.split(/[\r\n]+/).map(x => x.trim()).find(x => x.length > 0);

            if ((!cleanPayload || !cleanPayload.startsWith("http")) && htmlData) {
                const parser = new DOMParser();
                const doc = parser.parseFromString(htmlData, "text/html");
                const img = doc.querySelector("img");
                if (img && img.src) {
                    cleanPayload = img.src;
                }
            }

            if (!cleanPayload) return;

            console.log("Processing Drop:", cleanPayload);
            const isUrl = cleanPayload.startsWith("http");
            const isFile = cleanPayload.match(/^[a-zA-Z]:\\/) || cleanPayload.startsWith("file://") || cleanPayload.startsWith("/");
            if (!isUrl && !isFile) {
                return;
            }

            const result: any = await invoke('cache_dropped_item', { payload: cleanPayload });

            setEditorData({
                mode: 'create',
                filePath: result.temp_path,
                currentName: "New Sticker",
                currentPack: ["Recents", "Favorites", "All"].includes(activeTab) ? "" : activeTab
            });
        } catch (e) {
            console.error(e);
            alert("Failed to process image: " + e);
        }
    }, [activeTab]);

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
            const htmlData = e.dataTransfer?.getData("text/html");
            const payload = uriData || textData;

            if (payload || htmlData) {
                processDroppedPayload(payload || "", htmlData);
            }
        };

        const handlePaste = (e: ClipboardEvent) => {
            const target = e.target as HTMLElement;
            const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

            if (isInput) return;

            const textData = e.clipboardData?.getData("text/plain");
            const htmlData = e.clipboardData?.getData("text/html");

            if (textData || htmlData) {
                processDroppedPayload(textData || "", htmlData);
            }
        };

        window.addEventListener('dragenter', handleDragEnter);
        window.addEventListener('dragleave', handleDragLeave);
        window.addEventListener('dragover', handleDragOver);
        window.addEventListener('drop', handleDrop);
        window.addEventListener('paste', handlePaste);


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
            window.removeEventListener('paste', handlePaste);
            unlistenDrop.then(f => f());
            unlistenEnter.then(f => f());
            unlistenLeave.then(f => f());
        };
    }, [processDroppedPayload]);

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

    // wraps handle search so it searches all dirs not current
    const handleGlobalSearch = (val: string) => {
        if (val.length > 0 && activeTab !== "All") {
            handleTabClick("All");
        }
        handleSearch(val);
    };

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

            <div style={{ position: "relative", zIndex: 300 }}>
                <Header onToggleSettings={toggleSettings} onRefresh={reloadCurrentView} onClose={handleClose} />
            </div>
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
                        onSearch={handleGlobalSearch}
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