import { Component, type ReactNode } from "react";

/**
 * Keeps one widget's crash to that widget: it shows a small note (in edit
 * mode, so it can still be moved or hidden) instead of taking down every
 * widget and the edit bar with it. It tries again when the widget's data or
 * settings change.
 */
export class WidgetBoundary extends Component<{ name: string; editing: boolean; resetKey: unknown; children: ReactNode }, { error: Error | null; key: unknown }> {
  state = { error: null as Error | null, key: this.props.resetKey };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  static getDerivedStateFromProps(props: { resetKey: unknown }, state: { error: Error | null; key: unknown }) {
    return props.resetKey !== state.key ? { error: null, key: props.resetKey } : null;
  }

  componentDidCatch(error: Error) {
    console.error(`[tenths] the ${this.props.name} widget failed:`, error);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return this.props.editing ? <div className="panel widget-error">{this.props.name} couldn't show: {this.state.error.message}</div> : null;
  }
}
