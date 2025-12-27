import { useState, useEffect, useRef, useMemo, useLayoutEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, ask } from '@tauri-apps/plugin-dialog';
import { useVirtualizer } from '@tanstack/react-virtual';

const ITEM_MIN_WIDTH = 100;
const GAP = 15; 
const ROW_HEIGHT = 130;

interface Sticker {
  name: string;
  path: string;
  format: string;
  pack: string;
  is_favorite: boolean,
  rec_score: number,
}

interface AppSettings {
  sticker_path: string;
  recents_limit: number;
  theme: string;
}

function App() {
  const [stickers, setStickers] = useState<Sticker[]>([]);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState("All");
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<AppSettings>({
      sticker_path: "",
      recents_limit: 18,
      theme: "acrylic"
  });
  const [packs, setPacks] = useState<string[]>([]);
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

    // listens for the "app_shown" event from Rust
    const unlistenShown = listen("app_shown", () => {
        loadStickers(queryRef.current, activeTabRef.current); 
        
        // focus search bar
        setTimeout(() => {
            inputRef.current?.focus();
            inputRef.current?.select();
        }, 50);
    });

    const unlistenUpdate = listen("library_updated", () => {
        loadStickers(queryRef.current, activeTabRef.current); 
    });

    return () => {
        unlistenShown.then(f => f());
        unlistenUpdate.then(f => f());
    }
  }, []);

  useEffect(() => {
     invoke<string[]>("get_packs").then(setPacks);
  }, [stickers]);

  useLayoutEffect(() => {
    const updateColumns = () => {
      if (parentRef.current) {
        const width = parentRef.current.offsetWidth;
        // (width + gap) / (itemwidth + gap)
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

  const handleWipeData = async (type: "history" | "favorites") => {
      const confirmed = await ask(`Are you sure you want to wipe your ${type}?`, {
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
    // enter sends first sticker in list
    if (e.key === "Enter" && stickers.length > 0) {
        handleStickerClick(stickers[0].path);
    }
    // kicks you out
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
    estimateSize: () => ROW_HEIGHT, // (card + gap)
    overscan: 5,
  });

  return (
    <div style={{ 
      padding: "20px", height: "100%", width: "100%", overflow: "hidden", 
      boxSizing: "border-box", background: "transparent", display: "flex", flexDirection: "column", position: "relative"
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "20px" }}>
          <h1 style={{ margin: 0 }}>Soji</h1>
          <button 
            onClick={() => setShowSettings(!showSettings)}
            style={{
                background: "transparent", border: "none", color: "white", fontSize: "20px", cursor: "pointer", opacity: 0.7
            }}
          >
              ⚙
          </button>
      </div>

      {/* SETTINGS MODAL (Fixed Overlay) */}
      {showSettings && (
          <div style={{
              position: "fixed", top: 0, left: 0, width: "100%", height: "100%",
              background: "rgba(0, 0, 0, 0.6)", zIndex: 99,
              display: "flex", alignItems: "center", justifyContent: "center",
              backdropFilter: "blur(5px)"
          }} onClick={() => setShowSettings(false)}>
              
              <div 
                onClick={(e) => e.stopPropagation()} // Prevent click from closing modal
                style={{
                  width: "90%", maxWidth: "400px", maxHeight: "80%",
                  background: "rgba(30, 30, 30, 0.95)", borderRadius: "12px", padding: "25px",
                  display: "flex", flexDirection: "column", gap: "20px", 
                  border: "1px solid rgba(255,255,255,0.1)", boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
                  overflowY: "auto"
              }}>
                  <h2 style={{ margin: "0", borderBottom: "1px solid rgba(255,255,255,0.1)", paddingBottom: "10px" }}>Settings</h2>
                  
                  {/* GENERAL */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px" }}>Library</label>
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
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px" }}>Appearance</label>
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
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px" }}>Data</label>
                      
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                          <span style={{ fontSize: "14px" }}>Recents Limit</span>
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
                  </div>

                  <div style={{ marginTop: "auto", display: "flex", justifyContent: "flex-end" }}>
                      <button onClick={() => setShowSettings(false)} style={{ padding: "8px 20px", borderRadius: "5px", border: "1px solid rgba(255,255,255,0.2)", background: "transparent", color: "white", cursor: "pointer" }}>Close</button>
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
                {/* Total height container */}
                <div style={{ 
                    height: `${rowVirtualizer.getTotalSize()}px`, 
                    width: '100%', 
                    position: 'relative' 
                }}>
                    {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                        // Calculate which items belong in this row
                        const startIndex = virtualRow.index * columnCount;
                        const rowItems = stickers.slice(startIndex, startIndex + columnCount);

                        return (
                            <div
                                key={virtualRow.index}
                                style={{
                                    position: 'absolute',
                                    top: 0,
                                    left: 0,
                                    width: '100%',
                                    height: `${virtualRow.size}px`,
                                    transform: `translateY(${virtualRow.start}px)`,
                                    display: 'grid',
                                    gridTemplateColumns: `repeat(${columnCount}, 1fr)`,
                                    gap: `${GAP}px`,
                                    padding: '0 5px' // Slight side padding
                                }}
                            >
                                {rowItems.map((s, colIndex) => {
                                    // Highlight logic for keyboard nav
                                    const isFirst = (startIndex + colIndex) === 0 && query.length > 0;
                                    
                                    return (
                                        <div 
                                            key={s.path} 
                                            onClick={() => handleStickerClick(s.path)}
                                            onMouseEnter={(e) => e.currentTarget.style.background = "rgba(255, 255, 255, 0.2)"}
                                            onMouseLeave={(e) => e.currentTarget.style.background = isFirst ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)"}
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
                                                    position: "absolute", 
                                                    top: "5px", 
                                                    right: "5px", 
                                                    width: "24px", 
                                                    height: "24px", 
                                                    borderRadius: "50%", 
                                                    background: "rgba(0,0,0,0.3)", 
                                                    color: s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)", 
                                                    display: "flex", 
                                                    alignItems: "center", 
                                                    justifyContent: "center", 
                                                    fontSize: "14px", 
                                                    cursor: "pointer", 
                                                    zIndex: 10, 
                                                    transition: "all 0.2s" 
                                                }}
                                            >
                                                ♥
                                            </div>
                                            <img 
                                                src={convertFileSrc(s.path)} 
                                                alt={s.name} 
                                                style={{ width: "100%", height: "80px", objectFit: "contain", marginBottom: "5px" }} 
                                            />
                                            <span style={{ fontSize: "12px", textAlign: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "100%" }}>
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