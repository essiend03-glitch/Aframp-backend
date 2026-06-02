/**
 * React Error Boundary
 * Top-level error catching with localized recovery UI
 */

'use client';

import { Component, type ReactNode, type ErrorInfo } from 'react';
import { apiClient } from '@/lib/api/client';

interface Props {
  children: ReactNode;
  fallback?: (error: Error, errorInfo: ErrorInfo, reset: () => void) => ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
    };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return {
      hasError: true,
      error,
    };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    this.setState({
      error,
      errorInfo,
    });

    // Capture error context
    this.captureError(error, errorInfo);
  }

  private async captureError(error: Error, errorInfo: ErrorInfo): Promise<void> {
    try {
      const errorContext = {
        message: error.message,
        stack: error.stack,
        componentStack: errorInfo.componentStack,
        pathname: typeof window !== 'undefined' ? window.location.pathname : undefined,
        userAgent: typeof navigator !== 'undefined' ? navigator.userAgent : undefined,
        timestamp: new Date().toISOString(),
      };

      // Send to backend error tracking
      await apiClient.post('/api/v1/admin/infra/errors/capture', errorContext);
    } catch (captureError) {
      // Silently fail - don't throw in error boundary
      console.error('Failed to capture error:', captureError);
    }
  }

  private handleReset = (): void => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null,
    });
  };

  render(): ReactNode {
    if (this.state.hasError && this.state.error && this.state.errorInfo) {
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.state.errorInfo, this.handleReset);
      }

      return (
        <div className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
          <div className="max-w-md w-full bg-white rounded-lg shadow-lg p-8">
            <div className="flex items-center justify-center w-12 h-12 mx-auto bg-red-100 rounded-full mb-4">
              <svg
                className="w-6 h-6 text-red-600"
                fill="none"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="2"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>

            <h1 className="text-2xl font-bold text-gray-900 text-center mb-2">
              Something went wrong
            </h1>

            <p className="text-gray-600 text-center mb-6">
              We encountered an unexpected error. Our team has been notified and is working on a fix.
            </p>

            {process.env.NODE_ENV === 'development' && (
              <details className="mb-6 p-4 bg-gray-50 rounded border border-gray-200">
                <summary className="cursor-pointer font-medium text-sm text-gray-700 mb-2">
                  Error Details
                </summary>
                <div className="text-xs text-gray-600 space-y-2">
                  <div>
                    <strong>Message:</strong>
                    <pre className="mt-1 whitespace-pre-wrap">{this.state.error.message}</pre>
                  </div>
                  <div>
                    <strong>Stack:</strong>
                    <pre className="mt-1 whitespace-pre-wrap overflow-auto max-h-40">
                      {this.state.error.stack}
                    </pre>
                  </div>
                </div>
              </details>
            )}

            <div className="flex gap-3">
              <button
                onClick={this.handleReset}
                className="flex-1 bg-primary text-white py-2 px-4 rounded-md hover:bg-primary-hover transition-colors"
              >
                Try Again
              </button>
              <button
                onClick={() => {
                  if (typeof window !== 'undefined') {
                    window.location.href = '/';
                  }
                }}
                className="flex-1 bg-gray-200 text-gray-700 py-2 px-4 rounded-md hover:bg-gray-300 transition-colors"
              >
                Go Home
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
