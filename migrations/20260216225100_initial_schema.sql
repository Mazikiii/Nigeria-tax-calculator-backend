-- Add migration script here
-- Users table (core authentication)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT,
    auth_provider TEXT NOT NULL CHECK (auth_provider IN ('email', 'google', 'apple')),
    provider_id TEXT,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('PIT', 'LLC')),
    is_onboarded BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Pre-2026 assets
CREATE TABLE pre_2026_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    asset_name TEXT NOT NULL,
    asset_value BIGINT NOT NULL,
    valuation_date DATE NOT NULL DEFAULT '2025-12-31',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Existing loans
CREATE TABLE existing_loans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    lender_name TEXT NOT NULL,
    current_balance BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- LLC profiles
CREATE TABLE llc_profiles (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    total_fixed_assets BIGINT NOT NULL,
    tax_rate_category TEXT NOT NULL CHECK (tax_rate_category IN ('small', 'large')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tax states
CREATE TABLE user_tax_states (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tax_year INT NOT NULL,
    state_data JSONB NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (user_id, tax_year)
);

-- Processed statements
CREATE TABLE processed_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    batch_id UUID NOT NULL,
    tax_year INT NOT NULL,
    statement_data JSONB NOT NULL,
    processed_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_provider ON users(auth_provider, provider_id);
CREATE INDEX idx_assets_user ON pre_2026_assets(user_id);
CREATE INDEX idx_loans_user ON existing_loans(user_id);
CREATE INDEX idx_statements_user_year ON processed_statements(user_id, tax_year);
