using System;

namespace VamLibrary.SceneBrowser
{
    // Unity-independent state machine: reopening the same scene needs a real
    // loading transition, and a native confirmation dialog is allowed to defer it.
    public sealed class LaunchProgress
    {
        private readonly string before;
        private readonly string expected;
        private readonly float issuedAt;
        private bool sawLoading;
        public LaunchProgress(string before, string expected, float issuedAt)
        { this.before = before; this.expected = expected; this.issuedAt = issuedAt; }

        public string Observe(bool loading, string scene, float now)
        {
            if (loading) { sawLoading = true; return null; }
            if (sawLoading || !String.Equals(before, scene, StringComparison.OrdinalIgnoreCase))
                return String.Equals(expected, scene, StringComparison.OrdinalIgnoreCase) ? "loaded" : "load_failed";
            return now - issuedAt >= 60f ? "load_not_started" : null;
        }
    }
}
