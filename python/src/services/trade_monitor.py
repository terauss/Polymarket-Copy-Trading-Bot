"""
Trade monitor service - monitors trader activity via WebSocket
"""
import asyncio
import json
import websockets
from typing import List, Dict, Any, Optional
from ..config.env import ENV
from ..models.user_history import get_user_activity_collection, get_user_position_collection
from ..utils.fetch_data import fetch_data_async
from ..utils.logger import (
    info, success, warning, error, db_connection, my_positions,
    traders_positions, clear_line
)
from ..utils.get_my_balance import get_my_balance

USER_ADDRESSES = ENV.USER_ADDRESSES
TOO_OLD_TIMESTAMP = ENV.TOO_OLD_TIMESTAMP
RTDS_URL = 'wss://ws-live-data.polymarket.com'

if not USER_ADDRESSES or len(USER_ADDRESSES) == 0:
    raise ValueError('USER_ADDRESSES is not defined or empty')

# WebSocket connection state
ws: Optional[websockets.client.WebSocketClientProtocol] = None
reconnect_attempts = 0
MAX_RECONNECT_ATTEMPTS = 10
RECONNECT_DELAY = 5  # 5 seconds
is_running = True
position_update_task: Optional[asyncio.Task] = None
is_first_run = True


async def init():

async def process_trade_activity(activity: Dict[str, Any], address: str):


async def update_positions():
    

async def connect_rtds():



async def reconnect_loop():
   


def stop_trade_monitor():
    


async def trade_monitor():
    

