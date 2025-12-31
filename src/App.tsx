import { useState, useEffect, useRef, useLayoutEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, ask } from '@tauri-apps/plugin-dialog';
import { useVirtualizer } from '@tanstack/react-virtual';
import "./App.css";

const ITEM_MIN_WIDTH = 100;
const GAP = 15; 
const ROW_HEIGHT = 130;

interface Sticker {
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

interface AppSettings {
  sticker_path: string;
  recents_limit: number;
  theme: string;
}

interface IndexingProgress {
  current: number;
  total: number;
  eta_seconds: number | null;
}

function App() {
  const [stickers, setStickers] = useState<Sticker[]>([]);
  const [hoveredSticker, setHoveredSticker] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState("All");
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<AppSettings>({
      sticker_path: "",
      recents_limit: 18,
      theme: "acrylic"
  });
  const [packs, setPacks] = useState<string[]>([]);
  const [indexingProgress, setIndexingProgress] = useState<IndexingProgress | null>(null);
  const activeTabRef = useRef(activeTab);
  const queryRef = useRef(query);
  const parentRef = useRef<HTMLDivElement>(null);
  const [columnCount, setColumnCount] = useState(4);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => { activeTabRef.current = activeTab; }, [activeTab]);
  useEffect(() => { queryRef.current = query; }, [query]);

  useEffect(() => {
    refreshLibrary();
    invoke<AppSettings>("get_settings").then(setSettings);

    const unlistenShown = listen("app_shown", () => {
        loadStickers(queryRef.current, activeTabRef.current); 
        setTimeout(() => {
            inputRef.current?.focus();
            inputRef.current?.select();
        }, 50);
    });

    const unlistenUpdate = listen("library_updated", () => {
        setIndexingProgress(null);
        loadStickers(queryRef.current, activeTabRef.current);
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

  useLayoutEffect(() => {
    const updateColumns = () => {
      if (parentRef.current) {
        const width = parentRef.current.offsetWidth;
        const cols = Math.floor((width + GAP) / (ITEM_MIN_WIDTH + GAP));
        setColumnCount(Math.max(1, cols));
      }
    };

    updateColumns();
    const observer = new ResizeObserver(updateColumns);
    if (parentRef.current) observer.observe(parentRef.current);

    return () => observer.disconnect();
  }, [showSettings]);

  const refreshLibrary = async () => {
      await invoke("refresh_library");
      loadStickers(query, activeTab);
  };

  const loadStickers = async (searchQuery: string, currentTab: string) => {
    try {
        const result = await invoke<Sticker[]>("search_stickers", { 
            query: searchQuery, 
            tab: currentTab, 
            limit: 1000 
        });
        setStickers(result);
    } catch (err) {
        console.error(err);
    }
  };

  const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setQuery(val);
    loadStickers(val, activeTab);
  };

  const handleTabClick = (pack: string) => {
      setActiveTab(pack);
      loadStickers(query, pack);
  };

  const saveSettings = async (newSettings: AppSettings) => {
      setSettings(newSettings);
      await invoke("save_settings", { settings: newSettings });
      loadStickers(query, activeTab);
  };

  const handleChooseFolder = async () => {
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: settings.sticker_path || undefined,
        });

        if (selected) {
            saveSettings({ ...settings, sticker_path: selected as string });
        }
    } catch (err) {
        console.error(err);
    }
  };

  const handleThemeChange = async (theme: string) => {
    const newSettings = { ...settings, theme };
    saveSettings(newSettings);
  };

  const handleWipeData = async (type: "history" | "favorites" | "db") => {
      let message = `Are you sure you want to wipe your ${type}?`;
      if (type === "db") {
          message = "This will remove all stickers from the index and delete all cached thumbnails. The app will need to regenerate them, which may take time.";
      }

      const confirmed = await ask(message, {
          title: 'Confirm Wipe',
          kind: 'warning'
      });

      if (confirmed) {
          await invoke("wipe_data", { dataType: type });
          await invoke("refresh_library");
          setTimeout(() => loadStickers(query, activeTab), 100);
      }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && stickers.length > 0) {
        handleStickerClick(stickers[0].path);
    }
    if (e.key === "Escape") {
        invoke("hide_window").catch(() => {});
    }
  };

  const handleStickerClick = async (path: string) => {
    try {
      await invoke("select_sticker", { path });
    } catch (error) {
      console.error("Failed to select sticker:", error);
    }
  };

  const handleToggleFav = async (e: React.MouseEvent, path: string) => {
    e.stopPropagation();
    await invoke("toggle_favorite", { path });
    loadStickers(query, activeTab);
  };

  const rowCount = Math.ceil(stickers.length / columnCount);
  const rowVirtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 5,
  });

  const formatEta = (seconds: number | null) => {
      if (seconds === null) return "Calculating ETA...";
      if (seconds < 60) return `~${seconds}s remaining`;
      const mins = Math.ceil(seconds / 60);
      return `~${mins}m remaining`;
  };

  return (
    <div style={{ 
      padding: "20px", height: "100%", width: "100%", overflow: "hidden", 
      boxSizing: "border-box", background: "transparent", 
      display: "flex", flexDirection: "column", position: "relative",
      color: "white" // Force white text root
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "20px" }}>
          <h1 style={{ margin: 0, color: "white", fontSize: "28px", letterSpacing: "-1px" }}>Soji</h1>
          <button 
            onClick={() => setShowSettings(!showSettings)}
            style={{
                background: "transparent", border: "none", color: "white", fontSize: "24px", cursor: "pointer", opacity: 0.8
            }}
          >
              ⚙
          </button>
      </div>
      
      {/* LOADING OVERLAY */}
      {indexingProgress && (
          <div style={{
              position: "fixed", top: 0, left: 0, width: "100%", height: "100%",
              background: "rgba(0, 0, 0, 0.75)", // Darker for contrast
              zIndex: 200,
              display: "flex", alignItems: "center", justifyContent: "center",
              backdropFilter: "blur(20px)",
              color: "white" // Force white
          }}>
              <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "25px", textShadow: "0 2px 10px rgba(0,0,0,0.5)" }}>
                  <div className="spinner"></div>
                  <h2 style={{ margin: 0, fontSize: "26px", fontWeight: "400" }}>Indexing Images...</h2>
                  <div style={{ fontSize: "20px", opacity: 0.9, fontVariantNumeric: "tabular-nums", fontWeight: "bold" }}>
                      {indexingProgress.current} / {indexingProgress.total}
                  </div>
                  <div style={{ fontSize: "14px", opacity: 0.7 }}>
                      {formatEta(indexingProgress.eta_seconds)}
                  </div>
              </div>
          </div>
      )}
      
      {/* SETTINGS MODAL */}
      {showSettings && (
          <div style={{
              position: "fixed", top: 0, left: 0, width: "100%", height: "100%",
              background: "rgba(0, 0, 0, 0.6)", zIndex: 99,
              display: "flex", alignItems: "center", justifyContent: "center",
              backdropFilter: "blur(5px)"
          }} onClick={() => setShowSettings(false)}>
              
              <div 
                onClick={(e) => e.stopPropagation()} 
                style={{
                  width: "90%", maxWidth: "400px", maxHeight: "80%",
                  background: "rgba(30, 30, 30, 0.95)", borderRadius: "12px", padding: "25px",
                  display: "flex", flexDirection: "column", gap: "20px", 
                  border: "1px solid rgba(255,255,255,0.1)", boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
                  overflowY: "auto",
                  color: "white" // Force white text in modal
              }}>
                  <h2 style={{ margin: "0", borderBottom: "1px solid rgba(255,255,255,0.1)", paddingBottom: "10px", color: "white" }}>Settings</h2>
                  
                  {/* LIBRARY */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px", color: "white" }}>Library</label>
                      <div style={{ display: "flex", gap: "10px" }}>
                          <input 
                            readOnly 
                            value={settings.sticker_path || "Default (Pictures/Stickers)"} 
                            style={{ flex: 1, padding: "8px", borderRadius: "5px", border: "none", background: "rgba(0,0,0,0.5)", color: "white", fontSize: "12px" }}
                          />
                          <button onClick={handleChooseFolder} style={{ padding: "8px 15px", borderRadius: "5px", border: "none", background: "white", color: "black", cursor: "pointer", fontWeight: "bold" }}>Change</button>
                      </div>
                  </div>

                  {/* APPEARANCE */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px", color: "white" }}>Appearance</label>
                      <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                          <button 
                            onClick={() => handleThemeChange("acrylic")}
                            style={{ flex: 1, padding: "8px", borderRadius: "5px", border: "1px solid rgba(255,255,255,0.2)", 
                                     background: settings.theme === "acrylic" ? "white" : "transparent", 
                                     color: settings.theme === "acrylic" ? "black" : "white", cursor: "pointer" }}
                          >
                              Acrylic (Blur)
                          </button>
                          <button 
                            onClick={() => handleThemeChange("mica")}
                            style={{ flex: 1, padding: "8px", borderRadius: "5px", border: "1px solid rgba(255,255,255,0.2)", 
                                     background: settings.theme === "mica" ? "white" : "transparent", 
                                     color: settings.theme === "mica" ? "black" : "white", cursor: "pointer" }}
                          >
                              Mica (Tint)
                          </button>
                      </div>
                  </div>

                  {/* DATA */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px", color: "white" }}>Data</label>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                          <span style={{ fontSize: "14px", color: "white" }}>Recents Limit</span>
                          <input 
                            type="number" min="1" max="100"
                            value={settings.recents_limit}
                            onChange={(e) => saveSettings({...settings, recents_limit: parseInt(e.target.value) || 18})}
                            style={{ width: "60px", padding: "6px", borderRadius: "5px", border: "none", background: "rgba(0,0,0,0.5)", color: "white", textAlign: "center" }}
                          />
                      </div>
                      <div style={{ display: "flex", gap: "10px", marginTop: "5px" }}>
                          <button onClick={() => handleWipeData("history")} style={{ flex: 1, padding: "10px", borderRadius: "5px", border: "1px solid #ff4d4d", background: "rgba(255, 77, 77, 0.1)", color: "#ff4d4d", cursor: "pointer" }}>Wipe History</button>
                          <button onClick={() => handleWipeData("favorites")} style={{ flex: 1, padding: "10px", borderRadius: "5px", border: "1px solid #ff4d4d", background: "rgba(255, 77, 77, 0.1)", color: "#ff4d4d", cursor: "pointer" }}>Wipe Favs</button>
                      </div>
                      <button 
                        onClick={() => handleWipeData("db")} 
                        style={{ 
                            width: "100%", padding: "10px", borderRadius: "5px", 
                            border: "1px solid #ff4d4d", background: "rgba(255, 77, 77, 0.2)", 
                            color: "#ff4d4d", cursor: "pointer", fontWeight: "bold" 
                        }}
                      >
                        Reset Library & Cache
                      </button>
                  </div>
              </div>
          </div>
      )}

      {/* MAIN UI */}
      {!showSettings && (
          <>
            <input 
                ref={inputRef} type="text" placeholder="Search stickers..." value={query} onChange={handleSearch} onKeyDown={handleKeyDown}
                style={{ width: "100%", padding: "12px", marginBottom: "15px", borderRadius: "8px", border: "1px solid rgba(255,255,255,0.2)", background: "rgba(0,0,0,0.3)", color: "white", fontSize: "16px", outline: "none", backdropFilter: "blur(10px)" }}
            />

            {/* TABS */}
            {query.length === 0 && (
                <div className="no-scrollbar" style={{ display: "flex", gap: "8px", overflowX: "auto", paddingBottom: "10px", marginBottom: "5px", whiteSpace: "nowrap" }}>              
                    {["Recents", "Favorites", "All", ...packs].map(pack => (
                        <button key={pack} onClick={() => handleTabClick(pack)} style={{ padding: "6px 12px", borderRadius: "15px", border: "none", fontSize: "13px", cursor: "pointer", background: activeTab === pack ? "white" : "rgba(255,255,255,0.1)", color: activeTab === pack ? "black" : "white", transition: "all 0.2s", fontWeight: activeTab === pack ? "bold" : "normal" }}>{pack}</button>
                    ))}
                </div>
            )}
            
            {/* VIRTUALIZED GRID */}
            <div 
                ref={parentRef}
                style={{ flex: 1, overflowY: "auto", overflowX: "hidden", position: "relative" }}
            >
                <div style={{ 
                    height: `${rowVirtualizer.getTotalSize()}px`, 
                    width: '100%', 
                    position: 'relative' 
                }}>
                    {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                        const startIndex = virtualRow.index * columnCount;
                        const rowItems = stickers.slice(startIndex, startIndex + columnCount);

                        return (
                            <div
                                key={virtualRow.index}
                                style={{
                                    position: 'absolute',
                                    top: 0, left: 0, width: '100%',
                                    height: `${virtualRow.size}px`,
                                    transform: `translateY(${virtualRow.start}px)`,
                                    display: 'grid',
                                    gridTemplateColumns: `repeat(${columnCount}, 1fr)`,
                                    gap: `${GAP}px`,
                                    padding: '0 5px'
                                }}
                            >
                                {rowItems.map((s, colIndex) => {
                                    const isFirst = (startIndex + colIndex) === 0 && query.length > 0;
                                    const isHovered = hoveredSticker === s.id;
                                    const imgSrc = (s.format === 'gif' && isHovered) 
                                        ? s.path 
                                        : s.thumbnail_path;
                                    return (
                                        <div 
                                            key={s.id} 
                                            onClick={() => handleStickerClick(s.path)}
                                            onMouseEnter={(e) => {e.currentTarget.style.background = "rgba(255, 255, 255, 0.2)"; setHoveredSticker(s.id);}}
                                            onMouseLeave={(e) => {e.currentTarget.style.background = isFirst ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)"; setHoveredSticker(null);}}
                                            style={{ 
                                                display: "flex", 
                                                flexDirection: "column", 
                                                alignItems: "center", 
                                                padding: "10px", 
                                                background: isFirst ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)", 
                                                borderRadius: "8px", 
                                                backdropFilter: "blur(5px)", 
                                                cursor: "pointer", 
                                                transition: "background 0.2s",
                                                height: "100%",
                                                width: "100%",
                                                minWidth: 0,
                                                overflow: "hidden",
                                                boxSizing: "border-box"
                                            }} 
                                        >
                                            <div 
                                                onClick={(e) => handleToggleFav(e, s.path)} 
                                                onMouseEnter={(e) => e.currentTarget.style.color = "#ff4d4d"}
                                                onMouseLeave={(e) => e.currentTarget.style.color = s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)"}
                                                style={{ 
                                                    position: "absolute", top: "5px", right: "5px", 
                                                    width: "24px", height: "24px", borderRadius: "50%", 
                                                    background: "rgba(0,0,0,0.3)", 
                                                    color: s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)", 
                                                    display: "flex", alignItems: "center", justifyContent: "center", 
                                                    fontSize: "14px", cursor: "pointer", zIndex: 10, transition: "all 0.2s" 
                                                }}
                                            >
                                                ♥
                                            </div>
                                            <img 
                                                src={convertFileSrc(imgSrc)} 
                                                alt={s.name} 
                                                loading="eager"
                                                decoding="async"
                                                style={{ width: "100%", height: "80px", objectFit: "contain", marginBottom: "5px", contentVisibility: "auto" }} 
                                            />
                                            <span style={{ fontSize: "12px", textAlign: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "100%", color: "white" }}>
                                                {s.name}
                                            </span>
                                        </div>
                                    );
                                })}
                            </div>
                        );
                    })}
                </div>
            </div>
          </>
      )}
    </div>
  );
}

export default App;