import { createClient } from "@supabase/supabase-js";

const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL ?? "";
const SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY ?? "";
export const isSupabaseConfigured = Boolean(SUPABASE_URL && SUPABASE_ANON_KEY);

if (!isSupabaseConfigured) {
  console.warn(
    "Supabase not configured: set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY",
  );
}

function missingSupabaseError() {
  return new Error(
    "Supabase is not configured. Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY.",
  );
}

const fallbackClient = {
  auth: {
    async getSession() {
      return { data: { session: null }, error: null };
    },
    onAuthStateChange() {
      return {
        data: {
          subscription: {
            unsubscribe() {
              // no-op
            },
          },
        },
      };
    },
    async signInWithPassword() {
      return { error: missingSupabaseError() };
    },
    async signUp() {
      return { error: missingSupabaseError() };
    },
    async signInWithOAuth() {
      return { error: missingSupabaseError() };
    },
    async signOut() {
      return { error: null };
    },
  },
};

export const supabase = isSupabaseConfigured
  ? createClient(SUPABASE_URL, SUPABASE_ANON_KEY)
  : (fallbackClient as unknown as ReturnType<typeof createClient>);
