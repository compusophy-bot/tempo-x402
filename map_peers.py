import json, requests, os

def check_peers():
    try:
        with open('instance_info.json', 'r') as f:
            info = json.load(f)
        
        # Try to get siblings from local node first
        response = requests.get('http://localhost:8080/instance/siblings')
        if response.status_code != 200:
            print(f'Failed to get siblings: {response.status_code}')
            return
        
        siblings = response.json()
        reachable = []
        
        for peer in siblings:
            peer_url = peer.get('url')
            if not peer_url:
                continue
            
            try:
                # Check /info endpoint with 5s timeout
                info_resp = requests.get(f'{peer_url.rstrip("/")}/info', timeout=5)
                if info_resp.status_code == 200:
                    reachable.append({
                        'peer_id': peer.get('peer_id'),
                        'url': peer_url,
                        'status': 'reachable',
                        'info': info_resp.json()
                    })
            except Exception:
                continue
        
        with open('active_siblings.json', 'w') as f:
            json.dump(reachable, f, indent=2)
        
        print(f'Found {len(reachable)} reachable siblings.')
    except Exception as e:
        print(f'Error: {e}')

if __name__ == '__main__':
    check_peers()
