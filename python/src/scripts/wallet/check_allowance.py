#!/usr/bin/env python3
"""
Check and set USDC allowance for Polymarket trading
"""
import sys
from pathlib import Path

# Add project root to path
project_root = Path(__file__).parent.parent.parent.parent
if str(project_root) not in sys.path:
    sys.path.insert(0, str(project_root))

import asyncio
from web3 import Web3
from eth_account import Account
from src.config.env import ENV
from colorama import init, Fore, Style

init(autoreset=True)

# Polymarket Exchange address where tokens need to be approved
POLYMARKET_EXCHANGE = '0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E'
POLYMARKET_COLLATERAL = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174'  # USDC.e on Polygon
NATIVE_USDC_ADDRESS = '0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359'  # Native USDC on Polygon

# USDC ABI
USDC_ABI = [
    {
        "constant": True,
        "inputs": [{"name": "owner", "type": "address"}],
        "name": "balanceOf",
        "outputs": [{"name": "", "type": "uint256"}],
        "type": "function"
    },
    {
        "constant": True,
        "inputs": [
            {"name": "owner", "type": "address"},
            {"name": "spender", "type": "address"}
        ],
        "name": "allowance",
        "outputs": [{"name": "", "type": "uint256"}],
        "type": "function"
    },
    {
        "constant": False,
        "inputs": [
            {"name": "spender", "type": "address"},
            {"name": "amount", "type": "uint256"}
        ],
        "name": "approve",
        "outputs": [{"name": "", "type": "bool"}],
        "type": "function"
    },
    {
        "constant": True,
        "inputs": [],
        "name": "decimals",
        "outputs": [{"name": "", "type": "uint8"}],
        "type": "function"
    }
]


async def check_and_set_allowance():
    """Check and set USDC allowance"""
    print(f'{Fore.CYAN}[INFO]{Style.RESET_ALL} Checking USDC balance and allowance...\n')
    
    # Connect to Polygon
    w3 = Web3(Web3.HTTPProvider(ENV.RPC_URL))
    
    if not w3.is_connected():
        print(f'{Fore.RED}[ERROR]{Style.RESET_ALL} Failed to connect to RPC endpoint')
        sys.exit(1)
    
    # Create wallet from private key
    account = Account.from_key(ENV.PRIVATE_KEY)
    wallet_address = Web3.to_checksum_address(ENV.PROXY_WALLET)
    exchange_address = Web3.to_checksum_address(POLYMARKET_EXCHANGE)
    collateral_address = Web3.to_checksum_address(POLYMARKET_COLLATERAL)
    
    # Create USDC contract instance
    usdc_address = Web3.to_checksum_address(ENV.USDC_CONTRACT_ADDRESS)
    usdc_contract = w3.eth.contract(address=usdc_address, abi=USDC_ABI)
    
    try:
        # Get USDC decimals
        decimals = usdc_contract.functions.decimals().call()
        print(f'USDC Decimals: {decimals}')
        
        uses_polymarket_collateral = (usdc_address.lower() == collateral_address.lower())
        
        # Local token balance & allowance
        local_balance = usdc_contract.functions.balanceOf(wallet_address).call()
        local_allowance = usdc_contract.functions.allowance(wallet_address, exchange_address).call()
        local_balance_formatted = local_balance / (10 ** decimals)
        local_allowance_formatted = local_allowance / (10 ** decimals)
        
        print(f'Your USDC Balance ({ENV.USDC_CONTRACT_ADDRESS}): {local_balance_formatted:.6f} USDC')
        print(f'Current Allowance ({ENV.USDC_CONTRACT_ADDRESS}): {local_allowance_formatted:.6f} USDC')
        print(f'Polymarket Exchange: {POLYMARKET_EXCHANGE}\n')
        
        # Check native USDC if different
        if usdc_address.lower() != NATIVE_USDC_ADDRESS.lower():
            try:
                native_contract = w3.eth.contract(
                    address=Web3.to_checksum_address(NATIVE_USDC_ADDRESS),
                    abi=USDC_ABI
                )
                native_decimals = native_contract.functions.decimals().call()
                native_balance = native_contract.functions.balanceOf(wallet_address).call()
                if native_balance > 0:
                    native_formatted = native_balance / (10 ** native_decimals)
                    print(f'{Fore.YELLOW}[INFO]{Style.RESET_ALL} Detected native USDC (Polygon PoS) balance:')
                    print(f'    {native_formatted:.6f} tokens at {NATIVE_USDC_ADDRESS}')
                    print('    Polymarket does not recognize this token. Swap to USDC.e (0x2791...) to trade.\n')
            except Exception as native_error:
                print(f'{Fore.YELLOW}[WARNING]{Style.RESET_ALL} Unable to check native USDC balance: {native_error}')
        
        # Determine the contract Polymarket actually reads from (USDC.e)
        if uses_polymarket_collateral:
            polymarket_contract = usdc_contract
            polymarket_decimals = decimals
            polymarket_balance = local_balance
            polymarket_allowance = local_allowance
        else:
            polymarket_contract = w3.eth.contract(address=collateral_address, abi=USDC_ABI)
            polymarket_decimals = polymarket_contract.functions.decimals().call()
            polymarket_balance = polymarket_contract.functions.balanceOf(wallet_address).call()
            polymarket_allowance = polymarket_contract.functions.allowance(wallet_address, exchange_address).call()
        
        if not uses_polymarket_collateral:
            polymarket_balance_formatted = polymarket_balance / (10 ** polymarket_decimals)
            polymarket_allowance_formatted = polymarket_allowance / (10 ** polymarket_decimals)
            print(f'{Fore.YELLOW}[WARNING]{Style.RESET_ALL} Polymarket collateral token is USDC.e (bridged) at address')
            print(f'    {POLYMARKET_COLLATERAL}')
            print(f'Polymarket-tracked USDC balance: {polymarket_balance_formatted:.6f} USDC')
            print(f'Polymarket-tracked allowance: {polymarket_allowance_formatted:.6f} USDC\n')
            print('Swap native USDC to USDC.e or update your .env to point at the collateral token before trading.\n')
        
        if polymarket_allowance < polymarket_balance or polymarket_allowance == 0:
            print(f'{Fore.YELLOW}[WARNING]{Style.RESET_ALL} Allowance is insufficient or zero!')
            print('Setting unlimited allowance for Polymarket...\n')
            
            # Approve unlimited amount (max uint256)
            max_allowance = (2 ** 256) - 1
            
            # Get current gas price
            fee_data = w3.eth.fee_history(1, 'latest')
            if fee_data and fee_data.get('baseFeePerGas'):
                base_fee = fee_data['baseFeePerGas'][0]
                gas_price = int(base_fee * 1.5)  # 50% buffer
            else:
                gas_price = w3.to_wei(50, 'gwei')
            
            print(f'Gas Price: {w3.from_wei(gas_price, "gwei"):.2f} Gwei')
            
            # Build transaction
            nonce = w3.eth.get_transaction_count(wallet_address)
            approve_txn = polymarket_contract.functions.approve(exchange_address, max_allowance).build_transaction({
                'from': wallet_address,
                'gas': 100000,
                'gasPrice': gas_price,
                'nonce': nonce,
                'chainId': 137  # Polygon
            })
            
            # Sign transaction
            signed_txn = account.sign_transaction(approve_txn)
            
            # Send transaction
            tx_hash = w3.eth.send_raw_transaction(signed_txn.raw_transaction)
            print(f'Transaction sent: {tx_hash.hex()}')
            print('Waiting for confirmation...\n')
            
            # Wait for receipt
            receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=300)
            
            if receipt.status == 1:
                print(f'{Fore.GREEN}[SUCCESS]{Style.RESET_ALL} Allowance set successfully!')
                print(f'Transaction: https://polygonscan.com/tx/{tx_hash.hex()}\n')
                
                # Verify new allowance
                new_allowance = polymarket_contract.functions.allowance(wallet_address, exchange_address).call()
                new_allowance_formatted = new_allowance / (10 ** polymarket_decimals)
                print(f'{Fore.GREEN}[SUCCESS]{Style.RESET_ALL} New Allowance: {new_allowance_formatted:.6f} USDC')
            else:
                print(f'{Fore.RED}[ERROR]{Style.RESET_ALL} Transaction failed!')
        else:
            print(f'{Fore.GREEN}[SUCCESS]{Style.RESET_ALL} Allowance is already sufficient! No action needed.')
        
        print(f'\n{Fore.GREEN}[SUCCESS]{Style.RESET_ALL} Done!')
        
    except Exception as error:
        print(f'\n{Fore.RED}[ERROR]{Style.RESET_ALL} Error: {error}')
        if 'insufficient funds' in str(error).lower():
            print(f'\n{Fore.YELLOW}[WARNING]{Style.RESET_ALL} You need MATIC for gas fees on Polygon!')
        sys.exit(1)


if __name__ == '__main__':
    asyncio.run(check_and_set_allowance())

