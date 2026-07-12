import React, { useState, useEffect } from 'react';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import './StickerEditorModal.css';

export type EditorMode = 'create' | 'edit';

export interface EditorData {
    mode: EditorMode;
    filePath: string;
    currentName: string;
    currentPack: string;
}

interface Props {
    data: EditorData;
    packs: string[];
    onClose: () => void;
    onSuccess: () => void;
}

export const StickerEditorModal: React.FC<Props> = ({ data, packs, onClose, onSuccess }) => {
    const [name, setName] = useState(data.currentName);
    const [pack, setPack] = useState(data.currentPack);
    const [newPackName, setNewPackName] = useState("");
    const [isSubmitting, setIsSubmitting] = useState(false);

    useEffect(() => {
        const input = document.getElementById('sticker-name-input');
        if (input) input.focus();
    }, []);

    const handleSave = async () => {
        if (!name.trim() || (!pack && !newPackName.trim())) return;
        setIsSubmitting(true);

        const finalPack = newPackName.trim() || pack;

        try {
            if (data.mode === 'create') {
                await invoke('commit_sticker', {
                    tempPath: data.filePath,
                    name: name.trim(),
                    pack: finalPack
                });

            } else {
                const changes: any = { path: data.filePath };
                if (name.trim() !== data.currentName) changes.newName = name.trim();
                if (finalPack !== data.currentPack) changes.newPack = finalPack;

                await invoke('update_sticker', changes);
            }

            onSuccess();
            onClose();
        } catch (e) {
            console.error(e);
            alert(`Error saving sticker: ${e}`);
            setIsSubmitting(false);
        }
    };

    const handleOverlayClick = (e: React.MouseEvent) => {
        if (e.target === e.currentTarget) {
            onClose();
        }
    };

    const isVideo = React.useMemo(() => {
        const ext = data.filePath.split('.').pop()?.toLowerCase() || '';
        return ['mp4', 'webm', 'mov'].includes(ext);
    }, [data.filePath]);

    return (
        <div className="editor-overlay" onMouseDown={handleOverlayClick} onContextMenu={e => e.preventDefault()}>
            <div className="editor-modal" onMouseDown={(e) => e.stopPropagation()}>
                <h3 style={{ margin: 0, color: 'white' }}>
                    {data.mode === 'create' ? 'Add New Sticker' : 'Edit Sticker'}
                </h3>

                {/* IMAGE/VIDEO PREVIEW */}
                <div className="editor-preview-container">
                    {isVideo ? (
                        <video 
                            src={convertFileSrc(data.filePath)} 
                            className="editor-preview-img" 
                            autoPlay 
                            loop 
                            muted 
                            playsInline 
                        />
                    ) : (
                        <img 
                            src={convertFileSrc(data.filePath)} 
                            className="editor-preview-img" 
                            alt="Preview" 
                        />
                    )}
                </div>

                {/* NAME INPUT */}
                <div className="editor-field">
                    <label className="editor-label">Name</label>
                    <input
                        id="sticker-name-input"
                        className="editor-input"
                        value={name}
                        onChange={e => setName(e.target.value)}
                        placeholder="Sticker Name"
                        onKeyDown={e => e.key === 'Enter' && handleSave()}
                    />
                </div>

                {/* PACK SELECTION */}
                <div className="editor-field">
                    <label className="editor-label">Pack</label>
                    <div className="editor-pack-grid">
                        {packs.map(p => (
                            <button
                                key={p}
                                className={`pack-chip ${pack === p ? 'active' : ''}`}
                                onClick={() => { setPack(p); setNewPackName(""); }}
                            >
                                {p}
                            </button>
                        ))}
                    </div>
                    
                    {/* New Pack Input */}
                    <input 
                        className="editor-input"
                        style={{ marginTop: '5px', fontSize: '12px', padding: '6px' }}
                        placeholder={pack ? "(Selected: " + pack + ") or Type new pack name..." : "Type new pack name..."}
                        value={newPackName}
                        onChange={e => { setNewPackName(e.target.value); if(e.target.value) setPack(""); }}
                    />
                </div>

                {/* FOOTER */}
                <div className="editor-footer">
                    <button className="btn-cancel" onClick={onClose}>Cancel</button>
                    <button 
                        className="btn-save" 
                        onClick={handleSave}
                        disabled={isSubmitting || (!name.trim() || (!pack && !newPackName.trim()))}
                    >
                        {isSubmitting ? 'Saving...' : 'Save'}
                    </button>
                </div>
            </div>
        </div>
    );
};