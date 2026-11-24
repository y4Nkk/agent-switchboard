import { createContext } from "react";

// Portalled overlays need an explicit motion scope from their React owner.
export const OverlayMotionContext = createContext(false);
