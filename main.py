import streamlit as st
from imap_tools import MailBox, A
import pandas as pd
import plotly.express as px
import re
import concurrent.futures
import time

# --- CONFIGURATION ---
st.set_page_config(page_title="Inbox Assassin: Ultimate", page_icon="🥷", layout="wide")

MAX_THREADS = 10  # Gmail limit is usually ~15 connections per IP

# --- OPTIMIZATION 1 & 3: Regex Parsing on Raw Bytes ---
def fast_parse_sender(raw_bytes):
    """
    Rips the email address out of raw bytes using Regex.
    Bypasses the slow 'email' library entirely.
    """
    try:
        # 1. Decode loosely (ignore errors to go fast)
        text = raw_bytes.decode('utf-8', errors='ignore')
        
        # 2. Regex Search for 'From: Name <email@domain.com>' or 'From: email@domain.com'
        # This regex looks for the content after "From:"
        match = re.search(r'From:\s*(.*)', text, re.IGNORECASE)
        if match:
            raw_from = match.group(1).strip()
            # 3. Extract the actual email inside angle brackets if they exist
            email_match = re.search(r'<([^>]+)>', raw_from)
            if email_match:
                return email_match.group(1).lower()
            return raw_from.lower() # Fallback for "From: bob@aol.com"
        return "unknown"
    except:
        return "error"

# --- OPTIMIZATION 2: Threaded Worker ---
def scan_worker(creds, uid_batch):
    """
    Connects, fetches a specific batch of UIDs using SURGICAL FETCH, and parses.
    """
    results = []
    host = 'imap.gmail.com' # Default
    if 'outlook' in creds['user'] or 'hotmail' in creds['user']: host = 'imap-mail.outlook.com'
    elif 'yahoo' in creds['user']: host = 'imap.mail.yahoo.com'
    elif 'icloud' in creds['user']: host = 'imap.mail.me.com'

    try:
        # distinct connection for this thread
        with MailBox(host).login(creds['user'], creds['pass'], initial_folder=creds['folder']) as mb:
            # Join UIDs for the fetch command
            uid_str = ",".join(uid_batch)
            
            # OPTIMIZATION 1: Surgical Fetch
            # We ask ONLY for 'BODY.PEEK[HEADER.FIELDS (FROM)]'
            # We use 'uid fetch' to be precise
            responses = mb.client.uid('fetch', uid_str, '(BODY.PEEK[HEADER.FIELDS (FROM)])')
            
            if responses[0] == 'OK':
                # response format is messy list of tuples/bytes
                # logic: iterate through, look for the tuple containing the header data
                for item in responses[1]:
                    if isinstance(item, tuple):
                        # The raw header data is usually the second element: item[1]
                        sender = fast_parse_sender(item[1])
                        if sender and sender != "unknown":
                            results.append(sender)
    except Exception as e:
        print(f"Thread Error: {e}")
    return results

def get_all_uids(creds):
    """Get list of all UIDs quickly to split among threads."""
    host = 'imap.gmail.com'
    if 'outlook' in creds['user']: host = 'imap-mail.outlook.com'
    elif 'yahoo' in creds['user']: host = 'imap.mail.yahoo.com'
    elif 'icloud' in creds['user']: host = 'imap.mail.me.com'

    with MailBox(host).login(creds['user'], creds['pass'], initial_folder=creds['folder']) as mb:
        # imap_tools .uids() is optimized
        return mb.uids()

# --- OPTIMIZATION 4: Server-Side Deletion ---
def nuke_sender(creds, sender, action="trash"):
    host = 'imap.gmail.com'
    if 'outlook' in creds['user']: host = 'imap-mail.outlook.com'
    elif 'yahoo' in creds['user']: host = 'imap.mail.yahoo.com'
    elif 'icloud' in creds['user']: host = 'imap.mail.me.com'

    with MailBox(host).login(creds['user'], creds['pass'], initial_folder=creds['folder']) as mb:
        # 1. Server Side Search
        # This finds ALL emails from this sender, even ones we didn't scan
        msgs = mb.fetch(A(from_=sender), headers_only=True, bulk=True)
        uids = [msg.uid for msg in msgs]
        
        if not uids:
            return 0
            
        # 2. Batch Move/Delete
        # Splitting into chunks of 1000 for safety
        chunk_size = 1000
        for i in range(0, len(uids), chunk_size):
            batch = uids[i:i + chunk_size]
            if action == "trash":
                mb.move(batch, '[Gmail]/Trash')
            else:
                mb.delete(batch)
                
        return len(uids)

# --- UI LAYER ---
st.title("🥷 Inbox Assassin: Ultimate")
st.markdown("The fastest way to purge subscriptions and spam.")

if 'data' not in st.session_state:
    st.session_state['data'] = []

with st.sidebar:
    st.header("Credentials")
    email_user = st.text_input("Email", placeholder="you@gmail.com")
    email_pass = st.text_input("App Password", type="password")
    folder = st.text_input("Folder", value="INBOX")
    
    st.divider()
    scan_depth = st.slider("Scan Depth", min_value=0, max_value=50000, value=0, step=1)
    
    start_btn = st.button("🚀 Start Scan", type="primary")
    
    st.divider()
    delete_mode = st.radio("Mode", ["Move to Trash 🗑️", "Permanently Delete 💥"])
    action_code = "trash" if "Trash" in delete_mode else "delete"

if start_btn and email_user and email_pass:
    creds = {'user': email_user, 'pass': email_pass, 'folder': folder}
    st.session_state['creds'] = creds # Save for deletion usage
    
    status_cont = st.status("Initializing Assassin Protocols...", expanded=True)
    
    try:
        # 1. Fetch UIDs
        status_cont.write("🔍 Fetching Message IDs...")
        all_uids = get_all_uids(creds)
        total_found = len(all_uids)
        status_cont.write(f"Found {total_found} total emails.")
        
        # 2. Slice
        if scan_depth != "ALL":
            # Taking last N (newest)
            uids_to_process = all_uids[-scan_depth:]
        else:
            uids_to_process = all_uids
        
        count_to_process = len(uids_to_process)
        
        # 3. Batching & Threading
        status_cont.write(f"⚡ Spawning {MAX_THREADS} concurrent workers...")
        
        # Calculate chunk size
        chunk_size = (count_to_process // MAX_THREADS) + 1
        chunks = [uids_to_process[i:i + chunk_size] for i in range(0, count_to_process, chunk_size)]
        
        all_senders = []
        progress_bar = status_cont.progress(0)
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=MAX_THREADS) as executor:
            future_to_chunk = {executor.submit(scan_worker, creds, chunk): chunk for chunk in chunks}
            
            completed = 0
            for future in concurrent.futures.as_completed(future_to_chunk):
                data = future.result()
                all_senders.extend(data)
                completed += 1
                progress_bar.progress(completed / len(chunks))
                
        status_cont.write("✅ Aggregating Data...")
        st.session_state['data'] = all_senders
        status_cont.update(label="Scan Complete!", state="complete", expanded=False)
        st.rerun()

    except Exception as e:
        status_cont.update(label="Error!", state="error")
        st.error(f"Failed: {e}")

# --- DASHBOARD ---
if st.session_state['data']:
    df = pd.DataFrame(st.session_state['data'], columns=['sender'])
    
    # Simple Metrics
    col1, col2, col3 = st.columns(3)
    col1.metric("Emails Scanned", len(df))
    col2.metric("Unique Senders", df['sender'].nunique())
    
    # Identify Top Spammers
    counts = df['sender'].value_counts().reset_index()
    counts.columns = ['Sender', 'Count']
    
    # --- VISUALIZATION ---
    c_chart, c_action = st.columns([2, 1])
    
    with c_chart:
        st.subheader("Inbox Composition")
        fig = px.pie(
            counts.head(20), 
            values='Count', 
            names='Sender', 
            hole=0.4,
            color_discrete_sequence=px.colors.sequential.RdBu
        )
        fig.update_layout(margin=dict(t=20, b=0, l=0, r=0))
        st.plotly_chart(fig, use_container_width=True)

    with c_action:
        st.subheader("🎯 Kill List")
        
        # Filter Logic
        options = counts.head(100)['Sender'].tolist()
        selected = st.multiselect("Select Senders", options)
        
        if selected:
            # Calculate estimated count
            est_count = counts[counts['Sender'].isin(selected)]['Count'].sum()
            st.warning(f"You are about to remove ~{est_count} emails.")
            
            if st.button("EXECUTE", type="primary"):
                total_removed = 0
                progress_text = st.empty()
                bar = st.progress(0)
                
                for idx, sender in enumerate(selected):
                    progress_text.text(f"Purging {sender}...")
                    num = nuke_sender(st.session_state['creds'], sender, action_code)
                    total_removed += num
                    bar.progress((idx + 1) / len(selected))
                
                bar.empty()
                progress_text.empty()
                st.success(f"Successfully removed {total_removed} emails.")
                
                # Optimistic Update
                st.session_state['data'] = [s for s in st.session_state['data'] if s not in selected]
                st.rerun()

    # --- RAW DATA ---
    with st.expander("View Raw Data"):
        st.dataframe(counts)