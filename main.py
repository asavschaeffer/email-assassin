import streamlit as st
from imap_tools import MailBox, A, H
import pandas as pd
import plotly.express as px
import re
import requests
from datetime import datetime

# --- PAGE CONFIG ---
st.set_page_config(
    page_title="Inbox Assassin",
    page_icon="🧹",
    layout="wide",
    initial_sidebar_state="expanded"
)

# --- STYLING ---
st.markdown("""
<style>
    .stButton>button {
        width: 100%;
        border-radius: 5px;
        height: 3em;
    }
</style>
""", unsafe_allow_html=True)

# --- HELPER FUNCTIONS ---

def parse_unsubscribe_links(header_value):
    """
    Extracts HTTP and MAILTO links from the List-Unsubscribe header.
    Returns a dict with 'http' and 'mailto' keys.
    """
    if not header_value:
        return {'http': None, 'mailto': None}

    links = {'http': None, 'mailto': None}

    # Regex to find <url> pattern
    urls = re.findall(r'<([^>]+)>', header_value)
    for url in urls:
        if url.startswith('http'):
            links['http'] = url
        elif url.startswith('mailto'):
            links['mailto'] = url

    return links

def verify_credentials_and_folder(email_user, email_pass, folder):
    """
    Verifies that credentials work and the folder exists.
    Returns (success: bool, message: str, available_folders: list)
    """
    host = 'imap.gmail.com'
    if 'outlook' in email_user or 'hotmail' in email_user:
        host = 'imap-mail.outlook.com'
    elif 'yahoo' in email_user:
        host = 'imap.mail.yahoo.com'
    elif 'icloud' in email_user:
        host = 'imap.mail.me.com'

    try:
        with MailBox(host).login(email_user, email_pass) as mailbox:
            # Get list of all folders
            available_folders = [f.name for f in mailbox.folder.list()]

            # Check if requested folder exists
            if folder not in available_folders:
                return False, f"Folder '{folder}' not found.", available_folders

            # Try to select the folder
            mailbox.folder.set(folder)
            return True, "Credentials and folder verified successfully!", available_folders

    except Exception as e:
        error_msg = str(e)
        if "authentication failed" in error_msg.lower() or "invalid credentials" in error_msg.lower():
            return False, "❌ Invalid email or password. Make sure you're using an App Password, not your regular password.", []
        else:
            return False, f"❌ Connection error: {error_msg}", []

def fetch_emails(email_user, email_pass, folder, limit, progress_bar=None, status_text=None):
    """
    Connects to IMAP and fetches headers with progress updates.
    """
    email_data = []

    # Heuristic: Most providers use these hosts.
    # You can add a manual host input if needed.
    host = 'imap.gmail.com'
    if 'outlook' in email_user or 'hotmail' in email_user:
        host = 'imap-mail.outlook.com'
    elif 'yahoo' in email_user:
        host = 'imap.mail.yahoo.com'
    elif 'icloud' in email_user:
        host = 'imap.mail.me.com'

    try:
        if status_text:
            status_text.text(f"🔌 Connecting to {host}...")
        print(f"[DEBUG] Connecting to {host}...")

        with MailBox(host).login(email_user, email_pass, initial_folder=folder) as mailbox:
            if status_text:
                status_text.text(f"✅ Connected! Fetching emails...")
            print(f"[DEBUG] Connected successfully! Fetching up to {limit} emails...")

            # Fetch headers only (much faster than full emails)
            # Reverse = True to get newest first
            fetch_limit = None if limit == 0 else limit

            for i, msg in enumerate(mailbox.fetch(reverse=True, limit=fetch_limit, headers_only=True), 1):
                if i == 1:
                    print(f"[DEBUG] Successfully fetched first email! Continuing...")

                # Update progress bar
                if progress_bar and limit > 0:
                    progress_bar.progress(min(i / limit, 1.0))
                if status_text:
                    status_text.text(f"📧 Processing email {i}/{limit}...")

                if i % 100 == 0:
                    print(f"[DEBUG] Processed {i} emails so far...")

                # Clean sender name
                sender_name = msg.from_values.name
                sender_email = msg.from_values.email
                display_sender = f"{sender_name} <{sender_email}>" if sender_name else sender_email

                # Parse Unsubscribe Header
                unsub_header = msg.headers.get('list-unsubscribe', [None])[0]
                unsub_links = parse_unsubscribe_links(unsub_header)

                email_data.append({
                    "uid": msg.uid,
                    "sender_email": sender_email,
                    "display_sender": display_sender,
                    "subject": msg.subject,
                    "date": msg.date,
                    "unsub_http": unsub_links['http'],
                    "unsub_mailto": unsub_links['mailto'],
                    "size_kb": round(msg.size / 1024, 2)
                })

        if status_text:
            status_text.text(f"✅ Complete! Retrieved {len(email_data)} emails.")
        print(f"[DEBUG] Fetch complete! Retrieved {len(email_data)} emails.")
        return pd.DataFrame(email_data)
    except Exception as e:
        if status_text:
            status_text.text(f"❌ Error: {e}")
        print(f"[ERROR] Failed to fetch emails: {e}")
        return str(e)

def batch_delete(email_user, email_pass, folder, uids, action_type="trash"):
    """
    Deletes emails in batches to avoid server timeouts.
    """
    host = 'imap.gmail.com'
    if 'outlook' in email_user or 'hotmail' in email_user:
        host = 'imap-mail.outlook.com'
    elif 'yahoo' in email_user:
        host = 'imap.mail.yahoo.com'
    elif 'icloud' in email_user:
        host = 'imap.mail.me.com'

    try:
        with MailBox(host).login(email_user, email_pass, initial_folder=folder) as mailbox:
            # Chunk uids into batches of 50
            chunk_size = 50
            for i in range(0, len(uids), chunk_size):
                batch = uids[i:i + chunk_size]
                if action_type == "trash":
                    # Move to Trash (Safety Net)
                    # Note: Gmail specific trash folder usually works with [Gmail]/Trash
                    # For generic IMAP, 'delete' usually just adds a flag, but 'move' is safer
                    try:
                        mailbox.move(batch, '[Gmail]/Trash')
                    except:
                        # Fallback for non-Gmail or if folder name differs
                        mailbox.delete(batch)
                elif action_type == "permanent":
                    mailbox.delete(batch)
        return True
    except Exception as e:
        return str(e)

# --- SIDEBAR: LOGIN ---
with st.sidebar:
    st.header("🔐 Credentials")
    st.info("Use an App Password, not your login password.")
    email_user = st.text_input("Email Address")
    email_pass = st.text_input("App Password", type="password")
    folder = st.text_input("Folder to Scan", value="INBOX")
    scan_limit = st.slider("Max Emails to Scan", 500, 10000, 2000, step=500)
    
    start_btn = st.button("🚀 Connect & Scan", type="primary")
    
    st.markdown("---")
    st.markdown("### 🛑 Danger Zone")
    delete_mode = st.radio("Deletion Method", ["Move to Trash (Safe)", "Permanently Delete"])

# --- MAIN LOGIC ---

if start_btn:
    if not email_user or not email_pass:
        st.error("Please enter email and app password.")
    else:
        # Step 1: Verify credentials and folder
        with st.spinner("🔐 Verifying credentials and folder..."):
            success, message, available_folders = verify_credentials_and_folder(email_user, email_pass, folder)

        if not success:
            st.error(message)
            if available_folders:
                st.info(f"📁 Available folders: {', '.join(available_folders)}")
        else:
            st.success(message)

            # Step 2: Fetch emails with progress
            progress_bar = st.progress(0)
            status_text = st.empty()

            result = fetch_emails(email_user, email_pass, folder, scan_limit, progress_bar, status_text)

            if isinstance(result, str): # Error happened
                progress_bar.empty()
                status_text.empty()
                st.error(f"Connection Failed: {result}")
            else:
                # Initialize Session State
                st.session_state['df'] = result
                st.session_state['initial_count'] = len(result)
                progress_bar.empty()
                status_text.empty()
                st.rerun()

# --- DASHBOARD ---
if 'df' in st.session_state:
    df = st.session_state['df']
    
    # Top Metrics
    current_count = len(df)
    deleted_count = st.session_state['initial_count'] - current_count
    
    c1, c2, c3 = st.columns(3)
    c1.metric("Emails Scanned", current_count)
    c2.metric("Emails Purged", deleted_count)
    c3.metric("Unique Senders", df['sender_email'].nunique())
    
    st.divider()

    # --- VISUALIZATION ROW ---
    col_chart, col_list = st.columns([2, 1])

    with col_chart:
        st.subheader("📊 Who is clogging your inbox?")
        if not df.empty:
            # Group by sender
            sender_counts = df['sender_email'].value_counts().reset_index()
            sender_counts.columns = ['Sender', 'Count']
            
            # Pie Chart
            fig = px.pie(
                sender_counts.head(15),
                values='Count',
                names='Sender',
                hole=0.4,
                color_discrete_sequence=px.colors.qualitative.Set3
            )
            fig.update_traces(textposition='inside', textinfo='percent+label')
            fig.update_layout(margin=dict(t=0, b=0, l=0, r=0))
            st.plotly_chart(fig, use_container_width=True)
        else:
            st.info("Inbox is empty! Good job.")

    with col_list:
        st.subheader("🎯 Target Selection")
        if not df.empty:
            top_senders = df['sender_email'].value_counts().head(50).index.tolist()
            
            # Multi-select for deletion
            selected_senders = st.multiselect(
                "Select Senders to Remove:",
                options=top_senders,
                format_func=lambda x: f"{x} ({len(df[df['sender_email'] == x])} emails)"
            )
            
            # Calculate impact
            if selected_senders:
                msgs_to_delete = df[df['sender_email'].isin(selected_senders)]
                count_to_delete = len(msgs_to_delete)
                
                st.warning(f"⚠️ Selected {count_to_delete} emails from {len(selected_senders)} senders.")
                
                if st.button(f"🗑️ NUKE {count_to_delete} EMAILS"):
                    uids = msgs_to_delete['uid'].tolist()
                    
                    # 1. Perform IMAP Operation
                    with st.spinner("Deleting from server..."):
                        action = "permanent" if delete_mode == "Permanently Delete" else "trash"
                        status = batch_delete(email_user, email_pass, folder, uids, action)
                    
                    if status is True:
                        # 2. Optimistic Update (Remove from local state instantly)
                        st.session_state['df'] = df[~df['uid'].isin(uids)]
                        st.success(f"Successfully removed {count_to_delete} emails!")
                        st.rerun()
                    else:
                        st.error(f"Error during deletion: {status}")

    # --- DETAILED VIEW ---
    st.divider()
    with st.expander("🔎 Detailed Email View & Unsubscribe Links"):
        # Search Box
        search_term = st.text_input("Search Subject or Sender", "")
        
        # Filter Logic
        view_df = df.copy()
        if search_term:
            view_df = view_df[
                view_df['subject'].str.contains(search_term, case=False, na=False) | 
                view_df['sender_email'].str.contains(search_term, case=False, na=False)
            ]
        
        # Display Dataframe with Links
        st.dataframe(
            view_df[['date', 'display_sender', 'subject', 'unsub_http']],
            column_config={
                "unsub_http": st.column_config.LinkColumn("Unsubscribe Link")
            },
            use_container_width=True
        )