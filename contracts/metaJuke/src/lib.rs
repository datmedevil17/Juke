#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, BytesN, Env, Map, String, Symbol,
    Vec, Val, FromVal, IntoVal, TryFromVal, TryInto
};

// ----- Data Structures -----
#[contracttype]
#[derive(Clone)]
pub struct TableMembership {
    member: Address,
    joined_at: u64,
    is_admin: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct User {
    profile_nft: Address,
    avatar_uri: String,
    reputation: u32,
    is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct Artist {
    user_id: Address,
    artist_name: String,
    revenue_balance: i128,
    verified: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct JukeboxTable {
    table_id: BytesN<32>,
    name: String,
    owner: Address,
    current_track: Option<BytesN<32>>,
    queue: Vec<BytesN<32>>,
    skip_votes: Map<Address, bool>,
    skip_threshold: u32,
    price_multiplier: u32,
    member_count: u32,
    is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct Track {
    track_id: BytesN<32>,
    track_nft: Address,
    title: String,
    artist_id: Address,
    collaborators: Vec<Address>,
    play_count: u32,
    base_price: i128,
    licenses_remaining: u32,
    metadata_uri: String,
    royalty_split: Vec<(Address, u32)>,
}

#[contracttype]
#[derive(Clone)]
pub struct TrackRequest {
    request_id: BytesN<32>,
    requester: Address,
    track_id: BytesN<32>,
    table_id: BytesN<32>,
    timestamp: u64,
    amount_paid: i128,
}

#[contracttype]
pub enum ContractEvent {
    TrackMinted(BytesN<32>),
    TrackRequested(BytesN<32>),
    TableCreated(BytesN<32>),
    MembershipChanged(BytesN<32>, Address, bool, bool),
    AdminChanged(BytesN<32>, Address, bool),
    TableStatusChanged(BytesN<32>, bool),
    SkipVoted(BytesN<32>, Address),
}

#[contracttype]
enum DataKey {
    Admin,
    TokenStellar,
    Users(Address),
    Artists(Address),
    Tracks(BytesN<32>),
    Tables(BytesN<32>),
    Requests(BytesN<32>),
    UserToNft(Address),
    NftToUser(Address),
    PlatformFee,
    TrackIdCounter,
    TableIdCounter,
    RequestIdCounter,
    TableMembers(BytesN<32>, Address),
    TableAdmins(BytesN<32>, Address),
    UserTables(Address, BytesN<32>),
    ArtistTracks(Address, BytesN<32>),
    TableRequests(BytesN<32>, BytesN<32>),
}

#[contract]
pub struct MetaJuke;

#[contractimpl]
impl MetaJuke {
    pub fn initialize(env: Env, admin: Address, token_stellar: Address, platform_fee: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }
        
        admin.require_auth();
        
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenStellar, &token_stellar);
        env.storage().instance().set(&DataKey::PlatformFee, &platform_fee);
        env.storage().instance().set(&DataKey::TrackIdCounter, &0u32);
        env.storage().instance().set(&DataKey::TableIdCounter, &0u32);
        env.storage().instance().set(&DataKey::RequestIdCounter, &0u32);
    }
    
    pub fn update_platform_fee(env: Env, new_fee: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        
        if new_fee > 2000 {
            panic!("Fee too high");
        }
        
        env.storage().instance().set(&DataKey::PlatformFee, &new_fee);
    }
    
    pub fn register_user(env: Env, user: Address, profile_nft: Address, avatar_uri: String) {
        user.require_auth();
        
        if !Self::verify_nft_ownership(&env, &user, &profile_nft) {
            panic!("User doesn't own the NFT");
        }
        
        if env.storage().instance().has(&DataKey::Users(user.clone())) {
            panic!("User already registered");
        }
        
        if env.storage().instance().has(&DataKey::NftToUser(profile_nft.clone())) {
            panic!("NFT already associated with another user");
        }
        
        let new_user = User {
            profile_nft: profile_nft.clone(),
            avatar_uri,
            reputation: 100,
            is_active: true,
        };
        
        env.storage().instance().set(&DataKey::Users(user.clone()), &new_user);
        env.storage().instance().set(&DataKey::UserToNft(user.clone()), &profile_nft);
        env.storage().instance().set(&DataKey::NftToUser(profile_nft), &user);
    }
    
    pub fn register_artist(env: Env, user: Address, artist_name: String) {
        user.require_auth();
        
        if !env.storage().instance().has(&DataKey::Users(user.clone())) {
            panic!("User not registered");
        }
        
        if env.storage().instance().has(&DataKey::Artists(user.clone())) {
            panic!("Already registered as artist");
        }
        
        let new_artist = Artist {
            user_id: user.clone(),
            artist_name,
            revenue_balance: 0,
            verified: false,
        };
        
        env.storage().instance().set(&DataKey::Artists(user), &new_artist);
    }
    
    pub fn update_user_profile(env: Env, user: Address, avatar_uri: String) {
        user.require_auth();
        
        let mut user_data: User = env.storage().instance()
            .get(&DataKey::Users(user.clone()))
            .unwrap();
        
        user_data.avatar_uri = avatar_uri;
        env.storage().instance().set(&DataKey::Users(user), &user_data);
    }
    
    pub fn mint_track(
        env: Env,
        artist: Address,
        title: String,
        base_price: i128,
        licenses: u32,
        metadata_uri: String,
        collaborators: Vec<Address>,
        royalty_split: Vec<(Address, u32)>,
    ) -> BytesN<32> {
        artist.require_auth();
        
        if !env.storage().instance().has(&DataKey::Artists(artist.clone())) {
            panic!("Not registered as artist");
        }
        
        let mut total_split = 0;
        for (_, percentage) in royalty_split.iter() {
            total_split += percentage;
        }
        if total_split != 100 {
            panic!("Royalty splits must total 100%");
        }
        
        let mut track_counter: u32 = env.storage().instance()
            .get(&DataKey::TrackIdCounter)
            .unwrap();
        track_counter += 1;
        
        let track_id_str = String::from_str(&env, "track_");
        let mut track_id_bytes: Vec<u8> = track_id_str.to_string().into_bytes();
        track_id_bytes.extend_from_slice(&track_counter.to_be_bytes());
        let track_id = env.crypto().sha256(&track_id_bytes);

        let track_nft_str = String::from_str(&env, "track_nft_");
        let mut track_nft_id_bytes: Vec<u8> = track_nft_str.to_string().into_bytes();
        track_nft_id_bytes.extend_from_slice(&track_counter.to_be_bytes());
        let track_nft = Address::from_string(&String::from_str(&env, &String::from_utf8(track_nft_id_bytes).unwrap()));
        
        let new_track = Track {
            track_id: track_id.clone(),
            track_nft,
            title,
            artist_id: artist.clone(),
            collaborators,
            play_count: 0,
            base_price,
            licenses_remaining: licenses,
            metadata_uri,
            royalty_split,
        };
        
        env.storage().instance().set(&DataKey::Tracks(track_id.clone()), &new_track);
        env.storage().instance().set(
            &DataKey::ArtistTracks(artist.clone(), track_id.clone()),
            &true
        );
        env.storage().instance().set(&DataKey::TrackIdCounter, &track_counter);
        
        env.events().publish(
            (Symbol::new(&env, "track_minted"), track_id.clone()),
            ()
        );
        
        track_id
    }
    
    pub fn update_track(
        env: Env,
        artist: Address,
        track_id: BytesN<32>,
        new_base_price: i128,
        new_licenses: u32,
        new_metadata_uri: String,
    ) {
        artist.require_auth();
        
        let mut track: Track = env.storage().instance()
            .get(&DataKey::Tracks(track_id.clone()))
            .unwrap();
        
        if track.artist_id != artist {
            panic!("Not track owner");
        }
        
        track.base_price = new_base_price;
        track.licenses_remaining = new_licenses;
        track.metadata_uri = new_metadata_uri;
        
        env.storage().instance().set(&DataKey::Tracks(track_id), &track);
    }
    
    pub fn create_table(
        env: Env,
        owner: Address,
        name: String,
        skip_threshold: u32,
        price_multiplier: u32,
    ) -> BytesN<32> {
        owner.require_auth();
        
        if !env.storage().instance().has(&DataKey::Users(owner.clone())) {
            panic!("User not registered");
        }
        
        let mut table_counter: u32 = env.storage().instance()
            .get(&DataKey::TableIdCounter)
            .unwrap();
        table_counter += 1;
        
        let table_id_str = String::from_str(&env, "table_");
        let mut table_id_bytes: Vec<u8> = table_id_str.to_string().into_bytes();
        table_id_bytes.extend_from_slice(&owner.to_string().into_bytes());
        table_id_bytes.extend_from_slice(&table_counter.to_be_bytes());
        let table_id = env.crypto().sha256(&table_id_bytes);
                
        let new_table = JukeboxTable {
            table_id: table_id.clone(),
            name,
            owner: owner.clone(),
            current_track: None,
            queue: Vec::new(&env),
            skip_votes: Map::new(&env),
            skip_threshold,
            price_multiplier,
            member_count: 0,
            is_active: true,
        };
        
        env.storage().instance().set(&DataKey::Tables(table_id.clone()), &new_table);
        env.storage().instance().set(&DataKey::TableIdCounter, &table_counter);
        
        env.events().publish(
            (Symbol::new(&env, "table_created"), table_id.clone()),
            ()
        );
        
        table_id
    }
     
    pub fn update_table(
        env: Env,
        owner: Address,
        table_id: BytesN<32>,
        name: String,
        skip_threshold: u32,
        price_multiplier: u32,
    ) {
        owner.require_auth();
        
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.owner != owner {
            panic!("Not table owner");
        }
        
        table.name = name;
        table.skip_threshold = skip_threshold;
        table.price_multiplier = price_multiplier;
        
        env.storage().instance().set(&DataKey::Tables(table_id), &table);
    }
    
    pub fn request_track(
        env: Env,
        requester: Address,
        track_id: BytesN<32>,
        table_id: BytesN<32>,
    ) -> BytesN<32> {
        requester.require_auth();
        
        if !env.storage().instance().has(&DataKey::Users(requester.clone())) {
            panic!("User not registered");
        }
        
        if !env.storage().instance().has(&DataKey::TableMembers(table_id.clone(), requester.clone())) {
            panic!("Must be a table member to request tracks");
        }
        
        let mut track: Track = env.storage().instance()
            .get(&DataKey::Tracks(track_id.clone()))
            .unwrap();
        
        if track.licenses_remaining == 0 {
            panic!("No licenses remaining for this track");
        }
        
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        let base_price = track.base_price;
        let price_multiplier = table.price_multiplier;
        let final_price = (base_price * price_multiplier as i128) / 10000;
        
        let token_address: Address = env.storage().instance()
            .get(&DataKey::TokenStellar)
            .unwrap();
        let token_client = token::Client::new(&env, &token_address);
        
        token_client.transfer(
            &requester,
            &env.current_contract_address(),
            &final_price,
        );
        
        let mut request_counter: u32 = env.storage().instance()
            .get(&DataKey::RequestIdCounter)
            .unwrap();
        request_counter += 1;
        
        let request_id_str = String::from_str(&env, "request_");
        let mut request_id_bytes: Vec<u8> = request_id_str.to_string().into_bytes();
        request_id_bytes.extend_from_slice(&requester.to_string().into_bytes());
        request_id_bytes.extend_from_slice(&track_id.to_string().into_bytes());
        request_id_bytes.extend_from_slice(&env.ledger().timestamp().to_be_bytes());
        let request_id = env.crypto().sha256(&request_id_bytes);
        
        let new_request = TrackRequest {
            request_id: request_id.clone(),
            requester: requester.clone(),
            track_id: track_id.clone(),
            table_id: table_id.clone(),
            timestamp: env.ledger().timestamp(),
            amount_paid: final_price,
        };
        
        env.storage().instance().set(&DataKey::Requests(request_id.clone()), &new_request);
        env.storage().instance().set(&DataKey::RequestIdCounter, &request_counter);
        
        table.queue.push_back(track_id.clone());
        env.storage().instance().set(&DataKey::Tables(table_id), &table);
        
        track.licenses_remaining -= 1;
        track.play_count += 1;
        env.storage().instance().set(&DataKey::Tracks(track_id), &track);
        
        Self::distribute_royalties(&env, &track, &final_price);
        
        env.events().publish(
            (Symbol::new(&env, "track_requested"), request_id.clone()),
            ()
        );
        
        request_id
    }
    
    pub fn vote_to_skip(env: Env, user: Address, table_id: BytesN<32>) -> bool {
        user.require_auth();
        
        if !env.storage().instance().has(&DataKey::Users(user.clone())) {
            panic!("User not registered");
        }
        
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.current_track.is_none() {
            panic!("No track currently playing");
        }
        
        table.skip_votes.set(user.clone(), true);
        let vote_count = table.skip_votes.values().into_iter().filter(|&v| v).count();
        let should_skip = vote_count >= table.skip_threshold as usize;
        
        if should_skip {
            Self::advance_queue(&env, table_id.clone());
            table.skip_votes = Map::new(&env);
            env.storage().instance().set(&DataKey::Tables(table_id), &table);
            true
        } else {
            env.storage().instance().set(&DataKey::Tables(table_id), &table);
            false
        }
    }
    
    pub fn advance_queue(env: &Env, table_id: BytesN<32>) -> Option<BytesN<32>> {
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.queue.is_empty() {
            table.current_track = None;
            env.storage().instance().set(&DataKey::Tables(table_id.clone()), &table);
            return None;
        }
        
        let next_track = table.queue.pop_front().unwrap();
        table.current_track = Some(next_track.clone());
        table.skip_votes = Map::new(env);
        env.storage().instance().set(&DataKey::Tables(table_id.clone()), &table);
        
        Some(next_track)
    }
    
    fn distribute_royalties(env: &Env, track: &Track, payment_amount: &i128) {
        let platform_fee: u32 = env.storage().instance()
            .get(&DataKey::PlatformFee)
            .unwrap();
        
        let fee_amount = (payment_amount * platform_fee as i128) / 10000;
        let royalty_amount = payment_amount - fee_amount;
        
        let token_address: Address = env.storage().instance()
            .get(&DataKey::TokenStellar)
            .unwrap();
        let token_client = token::Client::new(env, &token_address);
        
        let admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap();
        token_client.transfer(
            &env.current_contract_address(),
            &admin,
            &fee_amount,
        );
        
        for (artist_address, percentage) in track.royalty_split.iter() {
            let artist_share = (royalty_amount * (percentage as i128)) / 100;
            
            if env.storage().instance().has(&DataKey::Artists(artist_address.clone())) {
                let mut artist: Artist = env.storage().instance()
                    .get(&DataKey::Artists(artist_address.clone()))
                    .unwrap();
                artist.revenue_balance += artist_share;
                env.storage().instance().set(&DataKey::Artists(artist_address.clone()), &artist);
            }
            
            token_client.transfer(
                &env.current_contract_address(),
                &artist_address,
                &artist_share,
            );
        }
    }
    
    pub fn withdraw_revenue(env: Env, artist: Address) -> i128 {
        artist.require_auth();
        
        if !env.storage().instance().has(&DataKey::Artists(artist.clone())) {
            panic!("Not registered as artist");
        }
        
        let mut artist_data: Artist = env.storage().instance()
            .get(&DataKey::Artists(artist.clone()))
            .unwrap();
        
        let amount = artist_data.revenue_balance;
        artist_data.revenue_balance = 0;
        env.storage().instance().set(&DataKey::Artists(artist.clone()), &artist_data);
        
        let token_address: Address = env.storage().instance()
            .get(&DataKey::TokenStellar)
            .unwrap();
        let token_client = token::Client::new(&env, &token_address);
        
        token_client.transfer(
            &env.current_contract_address(),
            &artist,
            &amount,
        );
        
        amount
    }
    
    fn verify_nft_ownership(env: &Env, user: &Address, nft_address: &Address) -> bool {
        let nft_client = token::Client::new(env, nft_address);
        
        // Check if NFT contract is valid
        let _symbol = nft_client.symbol();
        
        // Check balance
        let balance = nft_client.balance(user);
        if balance < 1 {
            return false;
        }
        
        // Check if NFT is already used
        if env.storage().instance().has(&DataKey::NftToUser(nft_address.clone())) {
            let existing_user: Address = env.storage().instance()
                .get(&DataKey::NftToUser(nft_address.clone()))
                .unwrap();
            return &existing_user == user;
        }
        
        true
    }
    
    // View functions
    pub fn get_user(env: Env, user: Address) -> Option<User> {
        env.storage().instance().get(&DataKey::Users(user))
    }
    
    pub fn get_artist(env: Env, artist: Address) -> Option<Artist> {
        env.storage().instance().get(&DataKey::Artists(artist))
    }
    
    pub fn get_track(env: Env, track_id: BytesN<32>) -> Option<Track> {
        env.storage().instance().get(&DataKey::Tracks(track_id))
    }
    
    pub fn get_table(env: Env, table_id: BytesN<32>) -> Option<JukeboxTable> {
        env.storage().instance().get(&DataKey::Tables(table_id))
    }
    
    pub fn get_queue(env: Env, table_id: BytesN<32>) -> Vec<BytesN<32>> {
        if let Some(table) = Self::get_table(env.clone(), table_id) {
            table.queue
        } else {
            Vec::new(&env)
        }
    }
    
    pub fn is_table_member(env: Env, user: Address, table_id: BytesN<32>) -> bool {
        env.storage().instance().has(&DataKey::TableMembers(table_id, user))
    }
    
    pub fn is_table_admin(env: Env, user: Address, table_id: BytesN<32>) -> bool {
        env.storage().instance().has(&DataKey::TableAdmins(table_id, user))
    }
    
    pub fn get_table_member_count(env: Env, table_id: BytesN<32>) -> u32 {
        if let Some(table) = Self::get_table(env, table_id) {
            table.member_count
        } else {
            0
        }
    }
    
    pub fn join_table(env: Env, user: Address, table_id: BytesN<32>) {
        user.require_auth();
        
        if !env.storage().instance().has(&DataKey::Users(user.clone())) {
            panic!("User not registered");
        }
        
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap_or_else(|| panic!("Table not found"));
        
        if !table.is_active {
            panic!("Table is closed");
        }
        
        if env.storage().instance().has(&DataKey::TableMembers(table_id.clone(), user.clone())) {
            panic!("Already a member of this table");
        }
        
        let membership = TableMembership {
            member: user.clone(),
            joined_at: env.ledger().timestamp(),
            is_admin: false,
        };
        
        env.storage().instance()
            .set(&DataKey::TableMembers(table_id.clone(), user.clone()), &membership);
        
        env.storage().instance()
            .set(&DataKey::UserTables(user.clone(), table_id.clone()), &true);
        
        table.member_count += 1;
        env.storage().instance()
            .set(&DataKey::Tables(table_id.clone()), &table);
        
        env.events().publish(
            (Symbol::new(&env, "membership_changed"), table_id.clone()),
            (user, true, false)
        );
    }
    
    pub fn leave_table(env: Env, user: Address, table_id: BytesN<32>) {
        user.require_auth();
        
        if !env.storage().instance().has(&DataKey::TableMembers(table_id.clone(), user.clone())) {
            panic!("Not a member of this table");
        }
        
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        env.storage().instance()
            .remove(&DataKey::TableMembers(table_id.clone(), user.clone()));
        
        table.member_count -= 1;
        env.storage().instance()
            .set(&DataKey::Tables(table_id.clone()), &table);
        
        env.events().publish(
            (Symbol::new(&env, "membership_changed"), table_id),
            (user, false, false)
        );
    }
    
    pub fn add_table_admin(env: Env, owner: Address, table_id: BytesN<32>, new_admin: Address) {
        owner.require_auth();
        
        let table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.owner != owner {
            panic!("Not table owner");
        }
        
        let admin_membership = TableMembership {
            member: new_admin.clone(),
            joined_at: env.ledger().timestamp(),
            is_admin: true,
        };
        
        env.storage().instance()
            .set(&DataKey::TableMembers(table_id.clone(), new_admin.clone()), &admin_membership);
        env.storage().instance()
            .set(&DataKey::TableAdmins(table_id, new_admin), &true);
    }
    
    pub fn remove_table_admin(env: Env, owner: Address, table_id: BytesN<32>, admin: Address) {
        owner.require_auth();
        
        let table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.owner != owner {
            panic!("Not table owner");
        }
        
        env.storage().instance()
            .remove(&DataKey::TableAdmins(table_id.clone(), admin.clone()));
        
        let membership = TableMembership {
            member: admin.clone(),
            joined_at: env.ledger().timestamp(),
            is_admin: false,
        };
        
        env.storage().instance()
            .set(&DataKey::TableMembers(table_id, admin), &membership);
    }
    
    pub fn set_table_status(env: Env, owner: Address, table_id: BytesN<32>, active: bool) {
        owner.require_auth();
        
        let mut table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.owner != owner {
            panic!("Not table owner");
        }
        
        table.is_active = active;
        
        if !active {
            table.queue = Vec::new(&env);
            table.current_track = None;
        }
        
        env.storage().instance()
            .set(&DataKey::Tables(table_id.clone()), &table);
        
        env.events().publish(
            (Symbol::new(&env, "table_status_changed"), table_id),
            active
        );
    }
    
    pub fn has_voted_to_skip(env: Env, user: Address, table_id: BytesN<32>) -> bool {
        let table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id))
            .unwrap();
        
        table.skip_votes.get(user).unwrap_or(false)
    }
    
    pub fn advance_queue_public(env: Env, caller: Address, table_id: BytesN<32>) -> Option<BytesN<32>> {
        caller.require_auth();
        
        let table: JukeboxTable = env.storage().instance()
            .get(&DataKey::Tables(table_id.clone()))
            .unwrap();
        
        if table.owner != caller {
            let membership: TableMembership = env.storage().instance()
                .get(&DataKey::TableMembers(table_id.clone(), caller))
                .unwrap_or_else(|| panic!("Not authorized"));
            
            if !membership.is_admin {
                panic!("Not authorized");
            }
        }
        
        Self::advance_queue(&env, table_id)
    }
    
    pub fn get_total_tracks(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TrackIdCounter).unwrap_or(0)
    }
    
    pub fn get_platform_stats(env: Env) -> (u32, u32, u32) {
        let total_tracks = Self::get_total_tracks(env.clone());
        let total_tables = env.storage().instance().get(&DataKey::TableIdCounter).unwrap_or(0);
        let total_requests = env.storage().instance().get(&DataKey::RequestIdCounter).unwrap_or(0);
        
        (total_tracks, total_tables, total_requests)
    }
}