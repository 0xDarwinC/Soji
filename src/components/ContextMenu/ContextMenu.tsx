import React, { useEffect, useRef } from 'react';
import './ContextMenu.css';

export interface ContextMenuCoords {
    x: number;
    y: number;
}

interface ContextMenuProps {
    coords: ContextMenuCoords;
    onClose: () => void;
    actions: {
        label: string;
        onClick: () => void;
        danger?: boolean;
        icon?: string;
    }[];
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ coords, onClose, actions }) => {
    const menuRef = useRef<HTMLDivElement>(null);

    // Close on click outside
    useEffect(() => {
        const handleClickOutside = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                onClose();
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [onClose]);

    // Ensure menu stays within viewport
    const style: React.CSSProperties = {
        top: coords.y,
        left: coords.x,
    };

    return (
        <div className="context-menu" ref={menuRef} style={style}>
            {actions.map((action, index) => (
                <React.Fragment key={index}>
                    <button 
                        className={`context-menu-item ${action.danger ? 'danger' : ''}`} 
                        onClick={() => {
                            action.onClick();
                            onClose();
                        }}
                    >
                        {action.icon && <span>{action.icon}</span>}
                        {action.label}
                    </button>
                </React.Fragment>
            ))}
        </div>
    );
};