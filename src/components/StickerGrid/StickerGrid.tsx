import React, { useRef, useLayoutEffect, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { convertFileSrc } from '@tauri-apps/api/core';
import { invoke } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';
import { Sticker } from '../../types';
import { ContextMenu, ContextMenuCoords } from '../ContextMenu/ContextMenu';

const ITEM_MIN_WIDTH = 90;
const GAP = 10;
const ROW_HEIGHT = 140;

interface StickerGridProps {
    stickers: Sticker[];
    packs: string[];
    onReload: () => void;
    onEdit: (sticker: Sticker) => void;
}

export const StickerGrid: React.FC<StickerGridProps> = ({ stickers, onReload, onEdit }) => {
    const parentRef = useRef<HTMLDivElement>(null);
    const [columnCount, setColumnCount] = useState(4);
    const [hoveredSticker, setHoveredSticker] = useState<number | null>(null);
    const [menuLocation, setMenuLocation] = useState<ContextMenuCoords | null>(null);
    const [menuTarget, setMenuTarget] = useState<Sticker | null>(null);

    useLayoutEffect(() => {
        const updateColumns = () => {
            if (parentRef.current) {
                const width = parentRef.current.offsetWidth - 40;
                const cols = Math.floor((width + GAP) / (ITEM_MIN_WIDTH + GAP));
                setColumnCount(Math.max(2, cols));
            }
        };
        updateColumns();
        const observer = new ResizeObserver(updateColumns);
        if (parentRef.current) observer.observe(parentRef.current);
        return () => observer.disconnect();
    }, []);

    const rowCount = Math.ceil(stickers.length / columnCount);
    const rowVirtualizer = useVirtualizer({
        count: rowCount,
        getScrollElement: () => parentRef.current,
        estimateSize: () => ROW_HEIGHT,
        overscan: 5,
    });

    const handleStickerClick = async (path: string) => {
        try {
            await invoke("select_sticker", { path });
        } catch (error) {
            console.error("Failed to select sticker:", error);
        }
    };

    const handleToggleFav = async (path: string) => {
        await invoke("toggle_favorite", { path });
        onReload();
    };

    const openMenu = (x: number, y: number, sticker: Sticker) => {
        setMenuLocation({ x, y });
        setMenuTarget(sticker);
    };

    const handleContainerContextMenu = (e: React.MouseEvent, sticker: Sticker) => {
        e.preventDefault();
        openMenu(e.clientX, e.clientY, sticker);
    };

    const handlePencilClick = (e: React.MouseEvent, sticker: Sticker) => {
        e.stopPropagation(); 
        e.preventDefault();
        onEdit(sticker);
    };

    const handleDelete = async () => {
        if (!menuTarget) return;
        const confirmed = await ask(`Are you sure you want to delete "${menuTarget.name} from the library"? The file will remain on disk.`, { 
            title: 'Delete Sticker', 
            kind: 'warning' 
        });
        
        if (confirmed) {
            try {
                await invoke("delete_sticker", { path: menuTarget.path });
                onReload();
            } catch (e) {
                console.error(e);
                alert("Failed to delete.");
            }
        }
    };

    return (
        <>
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
                                    gap: `${GAP}px`
                                }}
                            >
                                {rowItems.map((s) => {
                                    const isHovered = hoveredSticker === s.id;
                                    const imgSrc = (s.format === 'gif' && isHovered) ? s.path : s.thumbnail_path;
                                    return (
                                        <div
                                            key={s.id}
                                            className="sticker-container"
                                            onClick={() => handleStickerClick(s.path)}
                                            onMouseEnter={() => setHoveredSticker(s.id)}
                                            onMouseLeave={() => setHoveredSticker(null)}
                                            onContextMenu={(e) => handleContainerContextMenu(e, s)}
                                        >
                                            <div
                                                className="sticker-action-btn sticker-fav-btn"
                                                onClick={(e) => { e.stopPropagation(); handleToggleFav(s.path); }}
                                                style={{ color: s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.3)" }}>♥
                                            </div>
                                            <div className="sticker-action-btn sticker-edit-btn"
                                                onClick={(e) => handlePencilClick(e, s)} title="Edit">✎
                                            </div>
                                            <img src={convertFileSrc(imgSrc)} alt={s.name} loading="eager" decoding="async"
                                                style={{ width: "auto", maxWidth: "100%", height: "80px", objectFit: "contain", filter: "drop-shadow(0 4px 6px rgba(0,0,0,0.3))" }} />
                                            <div className="sticker-name-bubble">{s.name}</div>
                                        </div>
                                    );
                                })}
                            </div>
                        );
                    })}
                </div>
            </div>

            {menuLocation && menuTarget && (
                <ContextMenu 
                    coords={menuLocation}
                    onClose={() => setMenuLocation(null)}
                    actions={[
                        { 
                            label: menuTarget.is_favorite ? "Unfavorite" : "Favorite", 
                            onClick: () => handleToggleFav(menuTarget.path),
                            icon: "♥"
                        },
                        { 
                            label: "Edit / Move", 
                            onClick: () => onEdit(menuTarget), 
                            icon: "✎" 
                        },
                        { 
                            label: "Remove", 
                            onClick: handleDelete, 
                            danger: true, 
                            icon: "🗑" 
                        }
                    ]}
                />
            )}
        </>
    );
};