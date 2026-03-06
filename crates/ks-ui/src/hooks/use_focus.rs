//! Unified focus state management
//! Consolidates focus_area, selected indices, and sidebar focus into one signal

#![allow(dead_code)] // Some methods are reserved for future use

use dioxus::prelude::*;

/// Represents which UI zone currently has keyboard focus
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum FocusZone {
    /// Main resource list (default)
    #[default]
    MainList,
    /// Sidebar navigation
    Sidebar,
    /// Detail panel (logs, yaml viewer, container drilldown)
    DetailPanel,
    /// Modal dialog (delete confirmation)
    Modal,
    /// Search input is active
    Search,
    /// Namespace dropdown is open
    NamespaceDropdown,
}

/// Unified focus state management
#[derive(Clone, Default)]
pub struct FocusState {
    /// Current focus zone
    zone: FocusZone,
    /// Selected index in main list
    list_index: Option<usize>,
    /// Selected index in sidebar
    sidebar_index: Option<usize>,
    /// Selected button in modal (0=confirm, 1=cancel)
    modal_button_index: usize,
    /// Selected index in detail panel (e.g., container in drilldown)
    detail_index: Option<usize>,
}

impl FocusState {
    /// Get current focus zone
    pub fn zone(&self) -> FocusZone {
        self.zone
    }

    /// Check if a specific zone has focus
    pub fn is_focused(&self, zone: FocusZone) -> bool {
        self.zone == zone
    }

    /// Set the focus zone
    pub fn set_zone(&mut self, zone: FocusZone) {
        self.zone = zone;
    }

    /// Get list index
    pub fn list_index(&self) -> Option<usize> {
        self.list_index
    }

    /// Set list index
    pub fn set_list_index(&mut self, index: Option<usize>) {
        self.list_index = index;
    }

    /// Get sidebar index
    pub fn sidebar_index(&self) -> Option<usize> {
        self.sidebar_index
    }

    /// Set sidebar index
    pub fn set_sidebar_index(&mut self, index: Option<usize>) {
        self.sidebar_index = index;
    }

    /// Get detail index
    pub fn detail_index(&self) -> Option<usize> {
        self.detail_index
    }

    /// Set detail index
    pub fn set_detail_index(&mut self, index: Option<usize>) {
        self.detail_index = index;
    }

    /// Get modal button index
    pub fn modal_button_index(&self) -> usize {
        self.modal_button_index
    }

    /// Set modal button index
    pub fn set_modal_button_index(&mut self, index: usize) {
        self.modal_button_index = index;
    }

    /// Move focus left (typically to sidebar)
    pub fn move_left(&mut self) {
        match self.zone {
            FocusZone::MainList => {
                self.zone = FocusZone::Sidebar;
                // Initialize sidebar index if not set
                if self.sidebar_index.is_none() {
                    self.sidebar_index = Some(0);
                }
            }
            FocusZone::Modal => {
                // In modal, left goes to confirm button
                self.modal_button_index = 0;
            }
            _ => {}
        }
    }

    /// Move focus right (typically to main content)
    pub fn move_right(&mut self) {
        match self.zone {
            FocusZone::Sidebar => {
                self.zone = FocusZone::MainList;
            }
            FocusZone::Modal => {
                // In modal, right goes to cancel button
                self.modal_button_index = 1;
            }
            _ => {}
        }
    }

    /// Move selection up within the current zone
    pub fn move_up(&mut self, max_index: usize) {
        let index_ref = match self.zone {
            FocusZone::MainList => &mut self.list_index,
            FocusZone::Sidebar => &mut self.sidebar_index,
            FocusZone::DetailPanel => &mut self.detail_index,
            _ => return,
        };

        *index_ref = match *index_ref {
            Some(0) => Some(max_index.saturating_sub(1)), // Wrap to bottom
            Some(i) => Some(i.saturating_sub(1)),
            None => Some(0),
        };
    }

    /// Move selection down within the current zone
    pub fn move_down(&mut self, max_index: usize) {
        let index_ref = match self.zone {
            FocusZone::MainList => &mut self.list_index,
            FocusZone::Sidebar => &mut self.sidebar_index,
            FocusZone::DetailPanel => &mut self.detail_index,
            _ => return,
        };

        *index_ref = match *index_ref {
            Some(i) if i >= max_index.saturating_sub(1) => Some(0), // Wrap to top
            Some(i) => Some(i + 1),
            None => Some(0),
        };
    }

    /// Get the current index for the active zone
    pub fn current_index(&self) -> Option<usize> {
        match self.zone {
            FocusZone::MainList => self.list_index,
            FocusZone::Sidebar => self.sidebar_index,
            FocusZone::DetailPanel => self.detail_index,
            _ => None,
        }
    }

    /// Clear selection when switching contexts
    pub fn clear_list_selection(&mut self) {
        self.list_index = None;
    }

    /// Reset to default state
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Hook for unified focus state management
pub fn use_focus() -> Signal<FocusState> {
    use_signal(FocusState::default)
}
